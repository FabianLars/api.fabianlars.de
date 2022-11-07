use axum::{
    extract::Path,
    http::StatusCode,
    response::{self, IntoResponse},
    routing::get,
    Router,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use version_compare::Cmp;

pub fn router() -> Router {
    Router::new().route("/:app/:platform/:arch/:version", get(check_update))
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

async fn check_update(
    Path((app, platform, arch, version)): Path<(String, String, String, String)>,
) -> Result<impl IntoResponse, StatusCode> {
    if !["mw-toolbox"].contains(&app.as_str()) {
        log::error!("provided app name isn't supported: \"{}\"", &app);
        return Err(StatusCode::NOT_FOUND);
    };

    if !["darwin", "windows", "linux"].contains(&platform.as_str()) {
        log::error!(
            "provided platform doesn't match a supported value: \"{}\"",
            &platform
        );
        return Err(StatusCode::NOT_FOUND);
    };

    if !["x86_64", "aarch64"].contains(&arch.as_str()) {
        log::error!(
            "provided arch doesn't match a supported value: \"{}\"",
            &arch
        );
        return Err(StatusCode::NOT_FOUND);
    };

    let res = check_update_inner(app, platform, arch, version)
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
    arch: String,
    version: String,
) -> Result<UpdateResponse, anyhow::Error> {
    let mut dir = tokio::fs::read_dir(format!(
        "{}/releases/{}/latest/{}/{}/",
        &std::env::var("CDN_DIR")?,
        &app,
        &platform,
        &arch
    ))
    .await?;

    // Check for update
    while let Some(file) = dir.next_entry().await? {
        let file_path = file.path();
        if let Some(ext) = file_path.extension() {
            if ext == "sig" {
                if let Some(file_name) = file_path.file_name() {
                    let file_name = file_name.to_string_lossy();
                    let file_name_splits: Vec<&str> = file_name.split('_').collect();

                    if file_name_splits.len() >= 2
                        && version_compare::compare(file_name_splits[1], &version)
                            .unwrap_or(Cmp::Lt)
                            == Cmp::Gt
                    {
                        let created = file.metadata().await?.created()?;
                        let pub_date = DateTime::<Utc>::from(created).format("%+").to_string();

                        let signature = tokio::fs::read_to_string(&file_path).await?;

                        let url = format!(
                            "https://cdn.fabianlars.de/releases/{}/latest/{}/{}/{}",
                            &app,
                            &platform,
                            &arch,
                            &file_name.replace(".sig", "")
                        );

                        return Ok(UpdateResponse::Update(Update {
                            url,
                            version: file_name_splits[1].to_string(),
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
