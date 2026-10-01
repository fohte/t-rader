use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupAxis {
    pub id: Uuid,
    pub key: String,
    pub name: String,
    pub description: String,
    pub sync_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewGroupAxis {
    pub key: String,
    pub name: String,
    pub description: String,
    pub sync_source: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateGroupAxisCommand {
    pub key: String,
    pub name: String,
    pub description: String,
    pub sync_source: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateGroupAxisCommand {
    pub name: Option<String>,
    pub description: Option<String>,
    pub sync_source: Option<Option<String>>,
}
