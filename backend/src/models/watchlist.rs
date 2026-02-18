use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateWatchlistRequest {
    /// ウォッチリスト名
    #[schema(min_length = 1)]
    pub name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddWatchlistItemRequest {
    /// 銘柄コード (例: "7203")
    #[schema(min_length = 1)]
    pub instrument_id: String,
    /// 銘柄名 (例: "トヨタ自動車")
    #[schema(min_length = 1)]
    pub name: String,
}
