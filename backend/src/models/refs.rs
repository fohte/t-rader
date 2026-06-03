use serde::Serialize;
use utoipa::ToSchema;

/// `[[kind:id]]` のリンクテキストを解決した結果
#[derive(Debug, Serialize, ToSchema)]
pub struct RefResolution {
    /// "stock" | "indicator" | "sector" | "theme"
    pub kind: String,
    pub id: String,
    /// 一致しなかった場合は None
    pub name: Option<String>,
}
