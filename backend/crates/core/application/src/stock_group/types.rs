use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockGroupMembership {
    pub axis_key: String,
    pub group_key: String,
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
