use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateWatchlistRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct AddWatchlistItemRequest {
    pub instrument_id: String,
    pub name: String,
}
