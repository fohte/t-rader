/// 1 銘柄分のマスタ情報。`id` は 4 桁規約に正規化済み。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquityMasterEntry {
    pub id: String,
    pub name: String,
    pub market: Option<String>,
    pub sector_name: Option<String>,
    pub product_category: Option<String>,
}
