use std::{collections::HashMap, error::Error, net::SocketAddr};

use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::{self, IntoResponse},
    routing::get,
    Router,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tokio::fs::read;
use version_compare::Cmp;

#[derive(Deserialize, Serialize)]
struct Champ {
    name: String,
    codename: String,
    alias: String,
    id: i32,
    skins: Vec<Skin>,
}

#[derive(Deserialize, Serialize)]
struct Skin {
    id: i32,
    id_long: i32,
    name: String,
}

#[derive(Serialize)]
struct Rotation {
    start_date: NaiveDate,
    end_date: NaiveDate,
    champions: Vec<String>,
}

#[derive(Serialize)]
struct Update {
    url: String,
    version: String,
    notes: String,
    pub_date: String,
    signature: String,
}

enum UpdateResponse {
    Status(StatusCode),
    Update(Update),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;

    /*
        Route handler naming scheme:
        GET:    like rust getter functions, without a get_ prefix
        POST:   _create suffix
        PATCH:  _update suffix
        DELETE: _delete suffix
    */

    // League of Legends (wiki) related endpoints, listening on /v1/lol/...
    let lol_routes = Router::new()
        .route("/rotations", get(rotations))
        .route("/rotations/:id", get(rotation))
        .route("/rotations/latest", get(rotation_latest))
        .layer(Extension(pool)) // This adds the db pool to the routes created above
        .route("/champions", get(champions))
        .route("/champions/:id", get(champion));

    // Updater related endpoints, listening on /v1/update/...
    let updater = Router::new().route("/:app/:platform/:version", get(check_update));

    let app = Router::new()
        .nest("/v1/lol", lol_routes)
        .nest("/v1/update", updater);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3030));
    log::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

async fn champions() -> Result<impl IntoResponse, StatusCode> {
    let file = read("./champions.json")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // We could return the file straight from the filesystem, but let's check its validity first
    let json: HashMap<String, Champ> =
        serde_json::from_slice(&file).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(response::Json(json))
}

async fn champion(Path(id): Path<u16>) -> Result<impl IntoResponse, StatusCode> {
    let file = read("./champions.json")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut json: HashMap<String, Champ> =
        serde_json::from_slice(&file).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match json.remove(&id.to_string()) {
        Some(champ) => Ok(response::Json(champ)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn rotations(Extension(pool): Extension<PgPool>) -> Result<impl IntoResponse, StatusCode> {
    let rows = sqlx::query_as!(
        Rotation,
        "SELECT start_date, end_date, champions FROM rotations"
    )
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(response::Json(rows))
}
async fn rotation(
    Path(id): Path<u16>,
    Extension(pool): Extension<PgPool>,
) -> Result<impl IntoResponse, StatusCode> {
    let row = sqlx::query_as!(
        Rotation,
        "SELECT start_date, end_date, champions FROM rotations LIMIT 1 OFFSET $1",
        (id - 1) as i32
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => StatusCode::NOT_FOUND,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    })?;

    Ok(response::Json(row))
}

async fn rotation_latest(
    Extension(pool): Extension<PgPool>,
) -> Result<impl IntoResponse, StatusCode> {
    let row = sqlx::query_as!(
        Rotation,
        "SELECT start_date, end_date, champions FROM rotations ORDER BY id DESC LIMIT 1"
    )
    .fetch_one(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(response::Json(row))
}

async fn check_update(
    Path((app, platform, version)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, StatusCode> {
    if !["mw-toolbox"].contains(&app.as_str()) {
        log::error!("provided app name isn't supported: \"{}\"", &app);
        return Err(StatusCode::NOT_FOUND);
    };

    if !["darwin", "win64", "linux"].contains(&platform.as_str()) {
        log::error!(
            "provided platform doesn't match a supported value: \"{}\"",
            &platform
        );
        return Err(StatusCode::NOT_FOUND);
    };

    let res = check_update_inner(app, platform, version)
        .await
        .map_err(|err| {
            log::error!("Error: {:?}", err);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match res {
        // NO_CONTENT isn't actually an error, but it's simpler to return it as one
        UpdateResponse::Status(s) => Err(s),
        UpdateResponse::Update(u) => Ok(response::Json(u)),
    }
}

async fn check_update_inner(
    app: String,
    platform: String,
    version: String,
) -> Result<UpdateResponse, anyhow::Error> {
    let mut dir = tokio::fs::read_dir(format!(
        // Use only windows files to check for an update
        "{}/releases/{}/latest/win64/",
        &std::env::var("CDN_DIR")?,
        &app,
    ))
    .await?;

    let mut new_version: Option<String> = None;

    // Check for update in win64 folder
    while let Some(file) = dir.next_entry().await? {
        let file_path = file.path();
        if let Some(ext) = file_path.extension() {
            if ext == "sig" {
                if let Some(file_name) = file_path.file_name() {
                    let file_name = file_name.to_string_lossy();
                    let file_name_splits: Vec<&str> = file_name.split('_').collect();

                    if file_name_splits.len() >= 2
                        && version_compare::compare(&file_name_splits[1], &version)
                            .unwrap_or(Cmp::Lt)
                            == Cmp::Gt
                    {
                        new_version = Some(file_name_splits[1].to_string());
                    }
                }
            }
        }
    }

    // Generate response if update is available
    if let Some(new_version) = new_version {
        dir = tokio::fs::read_dir(format!(
            "{}/releases/{}/latest/{}/",
            &std::env::var("CDN_DIR")?,
            &app,
            &platform
        ))
        .await?;

        while let Some(file) = dir.next_entry().await? {
            let file_path = file.path();
            if let Some(ext) = file_path.extension() {
                if ext == "sig" {
                    if let Some(file_name) = file_path.file_name() {
                        let file_name = file_name.to_string_lossy();

                        let created = file.metadata().await?.created()?;
                        let pub_date = DateTime::<Utc>::from(created).format("%+").to_string();

                        let signature = tokio::fs::read_to_string(&file_path).await?;

                        let url = format!(
                            "https://cdn.fabianlars.de/releases/{}/latest/{}/{}",
                            &app,
                            &platform,
                            &file_name.replace(".sig", "")
                        );

                        return Ok(UpdateResponse::Update(Update {
                            url,
                            version: new_version,
                            notes: "No patch notes provided. You might want to check the project page on GitHub.".to_string(),
                            pub_date,
                            signature,
                        }));
                    }
                }
            }
        }
    }

    Ok(UpdateResponse::Status(StatusCode::NO_CONTENT))
}
