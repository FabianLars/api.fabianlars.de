use axum::Router;
use sqlx::PgPool;
use tokio::net::TcpListener;

mod ipsw;
#[cfg(feature = "league")]
mod league;
mod updater;

pub async fn serve(db: Option<PgPool>) -> anyhow::Result<()> {
    let app = api_router(db);

    let listener = TcpListener::bind("127.0.0.1:3030").await?;
    log::debug!("listening on 127.0.0.1:3030");
    axum::serve(listener, app).await?;

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
