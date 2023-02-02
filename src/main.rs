use api::http;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    #[cfg(feature = "league")]
    let db = PgPoolOptions::new()
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

    #[cfg(feature = "league")]
    http::serve(Some(db)).await?;
    #[cfg(not(feature = "league"))]
    http::serve(None).await?;

    Ok(())
}
