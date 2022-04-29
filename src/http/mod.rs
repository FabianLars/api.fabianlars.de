use std::net::SocketAddr;

use axum::Router;
use sqlx::PgPool;

mod ipsw;
mod league;
mod updater;

pub async fn serve(db: PgPool) -> anyhow::Result<()> {
    let app = api_router(db);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3030));
    log::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

fn api_router(db: PgPool) -> Router {
    Router::new()
        .nest("/v1/lol", league::router(db))
        .nest("/v1/update", updater::router())
}
