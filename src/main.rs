use std::path::PathBuf;

use chrono::{DateTime, NaiveDate, Utc};
use reqwest::StatusCode;
use serde::Serialize;
use sqlx::postgres::{PgPool, PgPoolOptions};
use version_compare::{CompOp, VersionCompare};
use warp::{
    reject,
    reply::{self, Json, WithStatus},
    Filter, Rejection,
};

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;

    /* Start of League of Legends Endpoints */

    let rotations = warp::path!("lol" / "rotations")
        .and(with_db(pool.clone()))
        .and_then(get_rotations);

    let rotation = warp::path!("lol" / "rotations" / u16)
        .and(with_db(pool.clone()))
        .and_then(get_rotation);

    let rotation_latest = warp::path!("lol" / "rotations" / "latest")
        .and(with_db(pool.clone()))
        .and_then(get_rotation_latest);

    let champions = warp::path!("lol" / "champions").and(warp::fs::file("./champions.json"));

    let champion = warp::path!("lol" / "champions" / u16).and_then(get_champion);

    /* End of League of Legends Endpoints */

    // mw-toolbox updater
    let update = warp::path!("update" / String / String / String).and_then(check_update);

    let v1 = warp::path!("v1" / ..).and(
        champions
            .or(champion)
            .or(rotations)
            .or(rotation)
            .or(rotation_latest)
            .or(update),
    );

    warp::serve(v1).run(([127, 0, 0, 1], 3030)).await;

    Ok(())
}

// Custom Filter to pipe PgPool into functions
fn with_db(
    pool: PgPool,
) -> impl Filter<Extract = (PgPool,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || pool.clone())
}

// endpoint
async fn get_champion(id: u16) -> Result<String, Rejection> {
    let file = tokio::fs::read_to_string("./champions.json")
        .await
        .map_err(|_| reject::not_found())?;
    let json: serde_json::Value = serde_json::from_str(&file).map_err(|_| reject::not_found())?;

    if let Some(champ) = json.get(id.to_string()) {
        Ok(champ.to_string())
    } else {
        Err(reject::not_found())
    }
}

// endpoint
async fn get_rotations(pool: PgPool) -> Result<Json, Rejection> {
    let rows = sqlx::query_as!(
        Rotation,
        "SELECT start_date, end_date, champions FROM rotations"
    )
    .fetch_all(&pool)
    .await
    .map_err(|_| reject::not_found())?;

    Ok(reply::json(&rows))
}

// endpoint
async fn get_rotation(id: u16, pool: PgPool) -> Result<Json, Rejection> {
    let row = sqlx::query_as!(
        Rotation,
        "SELECT start_date, end_date, champions FROM rotations LIMIT 1 OFFSET $1",
        (id - 1) as i32
    )
    .fetch_one(&pool)
    .await
    .map_err(|_| reject::not_found())?;

    Ok(reply::json(&row))
}

// endpoint
async fn get_rotation_latest(pool: PgPool) -> Result<Json, Rejection> {
    let row = sqlx::query_as!(
        Rotation,
        "SELECT start_date, end_date, champions FROM rotations ORDER BY id DESC LIMIT 1"
    )
    .fetch_one(&pool)
    .await
    .map_err(|_| reject::not_found())?;

    Ok(reply::json(&row))
}

// endpoint
async fn check_update(
    app: String,
    platform: String,
    version: String,
) -> Result<WithStatus<Json>, Rejection> {
    if !["mw-toolbox"].contains(&app.as_str()) {
        log::error!("provided app name isn't supported: \"{}\"", &app);
        return Err(reject::not_found());
    };

    if !["darwin", "win64", "linux"].contains(&platform.as_str()) {
        log::error!(
            "provided platform doesn't match a supported value: \"{}\"",
            &platform
        );
        return Err(reject::not_found());
    };

    let res = check_update_inner(app, platform, version).await;

    Ok(match res {
        Ok(r) => r,
        Err(err) => {
            log::error!("Error: {:?}", err);
            reply::with_status(reply::json(&"".to_string()), StatusCode::NO_CONTENT)
        }
    })
}

// helper
async fn check_update_inner(
    app: String,
    platform: String,
    version: String,
) -> Result<WithStatus<Json>, anyhow::Error> {
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
                        && VersionCompare::compare(&file_name_splits[1], &version)
                            .unwrap_or(CompOp::Lt)
                            == CompOp::Gt
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

                        return Ok(reply::with_status(reply::json(&Update {
                            url,
                            version: new_version,
                            notes: "No patch notes provided. You might want to check the project page on GitHub.".to_string(),
                            pub_date,
                            signature,
                        }), StatusCode::OK));
                    }
                }
            }
        }
    }

    Ok(reply::with_status(
        reply::json(&"No update available"),
        StatusCode::NO_CONTENT,
    ))
}
