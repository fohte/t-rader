use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, FromRow)]
pub struct Watchlist {
    pub id: Uuid,
    pub name: String,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWatchlistRequest {
    pub name: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Instrument {
    pub id: String,
    pub name: String,
    pub market: String,
    pub sector: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct WatchlistItem {
    pub watchlist_id: Uuid,
    pub instrument_id: String,
    pub sort_order: i32,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AddWatchlistItemRequest {
    pub instrument_id: String,
    pub name: String,
}
