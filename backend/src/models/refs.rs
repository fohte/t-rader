use serde::Serialize;
use utoipa::ToSchema;

/// `[[kind:id]]` のリンクテキストを解決した結果
#[derive(Debug, PartialEq, Eq, Serialize, ToSchema)]
pub struct RefResolution {
    /// "stock" | "indicator" | "sector" | "theme"
    pub kind: String,
    /// 別名で解決できた場合、入力ではなく正規の id
    pub id: String,
    /// 一致しなかった場合は None
    pub name: Option<String>,
}
