use chrono::NaiveDate;
use serde::Serialize;
use sqlx::postgres::{PgPool, PgPoolOptions};
use warp::{reply::Json, Filter, Rejection};

mod docs;

#[derive(Serialize)]
struct Rotation {
    start_date: NaiveDate,
    end_date: NaiveDate,
    champions: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;

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

    let v1 = warp::path!("v1" / ..).and(
        champions
            .or(champion)
            .or(rotations)
            .or(rotation)
            .or(rotation_latest),
    );

    warp::serve(docs::openapi_docs().or(v1))
        .run(([127, 0, 0, 1], 3030))
        .await;

    Ok(())
}

// Custom Filter to pipe PgPool into functions
fn with_db(
    pool: PgPool,
) -> impl Filter<Extract = (PgPool,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || pool.clone())
}

async fn get_champion(id: u16) -> Result<String, Rejection> {
    let file = tokio::fs::read_to_string("./champions.json")
        .await
        .map_err(|_| warp::reject::not_found())?;
    let json: serde_json::Value =
        serde_json::from_str(&file).map_err(|_| warp::reject::not_found())?;

    if let Some(champ) = json.get(id.to_string()) {
        Ok(champ.to_string())
    } else {
        Err(warp::reject::not_found())
    }
}

async fn get_rotations(pool: PgPool) -> Result<Json, Rejection> {
    let rows = sqlx::query_as!(
        Rotation,
        "SELECT start_date, end_date, champions FROM rotations"
    )
    .fetch_all(&pool)
    .await
    .map_err(|_| warp::reject::not_found())?;

    Ok(warp::reply::json(&rows))
}

async fn get_rotation(id: u16, pool: PgPool) -> Result<Json, Rejection> {
    let row = sqlx::query_as!(
        Rotation,
        "SELECT start_date, end_date, champions FROM rotations LIMIT 1 OFFSET $1",
        (id - 1) as i32
    )
    .fetch_one(&pool)
    .await
    .map_err(|_| warp::reject::not_found())?;

    Ok(warp::reply::json(&row))
}

async fn get_rotation_latest(pool: PgPool) -> Result<Json, Rejection> {
    let row = sqlx::query_as!(
        Rotation,
        "SELECT start_date, end_date, champions FROM rotations ORDER BY id DESC LIMIT 1"
    )
    .fetch_one(&pool)
    .await
    .map_err(|_| warp::reject::not_found())?;

    Ok(warp::reply::json(&row))
}