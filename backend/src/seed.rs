use sqlx::{Row, SqlitePool};

use crate::config::Config;

mod catalog;
mod contract;
mod creator;
mod local;
mod playback;
mod support;
mod user;

use catalog::{seed_categories, seed_films, seed_live_streams, seed_series, seed_streamers};
use contract::ensure_extended_contract_data;
use creator::seed_creator;
use local::seed_local_auth_session;
use user::{seed_chat, seed_users};

pub async fn seed_if_empty(pool: &SqlitePool, config: &Config) -> Result<(), sqlx::Error> {
    let count: i64 = sqlx::query("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?
        .get(0);

    if count != 0 {
        return Ok(());
    }

    seed_series(pool).await?;
    seed_films(pool).await?;
    seed_streamers(pool).await?;
    seed_live_streams(pool).await?;
    seed_categories(pool).await?;
    seed_users(pool).await?;
    seed_creator(pool).await?;
    seed_chat(pool).await?;
    ensure_extended_contract_data(pool).await?;
    seed_local_auth_session(pool, config).await?;

    Ok(())
}
