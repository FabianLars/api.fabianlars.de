use std::net::SocketAddr;

use axum::Router;
use sqlx::PgPool;

mod ipsw;
#[cfg(feature = "league")]
mod league;
mod updater;

pub async fn serve(db: Option<PgPool>) -> anyhow::Result<()> {
    let app = api_router(db);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3030));
    log::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

fn api_router(_db: Option<PgPool>) -> Router {
    let mut router = Router::new();

    router = router.nest("/v1/update", updater::router());

    #[cfg(feature = "league")]
    {
        router = router.nest("/v1/lol", league::router(_db.unwrap()));
    }

    router
}
