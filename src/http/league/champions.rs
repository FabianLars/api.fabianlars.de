use std::collections::HashMap;

use axum::{
    extract::Path,
    http::StatusCode,
    response::{self, IntoResponse},
};
use serde::{Deserialize, Serialize};
use tokio::fs::read;

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

pub async fn all() -> Result<impl IntoResponse, StatusCode> {
    let file = read("./champions.json")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // We could return the file straight from the filesystem, but let's check its validity first
    let json: HashMap<String, Champ> =
        serde_json::from_slice(&file).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(response::Json(json))
}

pub async fn one(Path(id): Path<u16>) -> Result<impl IntoResponse, StatusCode> {
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
