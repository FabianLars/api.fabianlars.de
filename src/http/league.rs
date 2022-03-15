use axum::{extract::Extension, routing::get, Router};
use sqlx::PgPool;

mod champions;
mod rotations;

pub fn router(db: PgPool) -> Router {
    Router::new()
        .route("/rotations", get(rotations::all))
        .route("/rotations/:id", get(rotations::one))
        .route("/rotations/latest", get(rotations::latest))
        .layer(Extension(db)) // This adds the db pool to the routes created above
        .route("/champions", get(champions::all))
        .route("/champions/:id", get(champions::one))
}
