use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StockGroupCodeLookup {
    NotFound,
    Missing,
    Found(String),
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockGroupMembership {
    pub axis_key: String,
    pub axis_name: String,
    pub group_key: String,
    pub group_name: String,
    pub stock_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockGroup {
    pub id: Uuid,
    pub axis_id: Uuid,
    pub axis_key: String,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockGroupStock {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockGroupDetail {
    pub group: StockGroup,
    pub axis_name: String,
    pub stocks: Vec<StockGroupStock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewStockGroup {
    pub id: Uuid,
    pub axis_id: Uuid,
    pub axis_key: String,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateStockGroupCommand {
    pub axis_key: String,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateStockGroupCommand {
    pub axis_key: String,
    pub key: String,
    pub name: Option<String>,
    pub description: Option<Option<String>>,
}
