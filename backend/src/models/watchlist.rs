use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateWatchlistRequest {
    /// ウォッチリスト名
    // trim 後に空文字列になる入力 (制御文字のみ等) をスキーマレベルで排除する
    #[schema(min_length = 1, pattern = r"\S")]
    pub name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddWatchlistItemRequest {
    /// 銘柄コード (例: "7203")
    // trim 後に空文字列になる入力 (制御文字のみ等) をスキーマレベルで排除する
    #[schema(min_length = 1, pattern = r"\S")]
    pub instrument_id: String,
    /// 銘柄名 (例: "トヨタ自動車")
    // trim 後に空文字列になる入力 (制御文字のみ等) をスキーマレベルで排除する
    #[schema(min_length = 1, pattern = r"\S")]
    pub name: String,
}
