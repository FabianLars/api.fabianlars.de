use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::{self, IntoResponse},
};
use chrono::NaiveDate;
use serde::Serialize;
use sqlx::PgPool;

#[derive(Serialize)]
struct Rotation {
    start_date: NaiveDate,
    end_date: NaiveDate,
    champions: Vec<String>,
}

pub async fn all(Extension(pool): Extension<PgPool>) -> Result<impl IntoResponse, StatusCode> {
    let rows = sqlx::query_as!(
        Rotation,
        "SELECT start_date, end_date, champions FROM rotations"
    )
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(response::Json(rows))
}
pub async fn one(
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

pub async fn latest(Extension(pool): Extension<PgPool>) -> Result<impl IntoResponse, StatusCode> {
    let row = sqlx::query_as!(
        Rotation,
        "SELECT start_date, end_date, champions FROM rotations ORDER BY id DESC LIMIT 1"
    )
    .fetch_one(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(response::Json(row))
}
