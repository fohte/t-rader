use serde::Serialize;
use utoipa::ToSchema;

use core_application::group_axis::GroupAxis;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = GroupAxis)]
pub struct GroupAxisResponse {
    pub key: String,
    pub name: String,
    pub description: String,
    pub sync_source: Option<String>,
}

impl From<GroupAxis> for GroupAxisResponse {
    fn from(axis: GroupAxis) -> Self {
        Self {
            key: axis.key,
            name: axis.name,
            description: axis.description,
            sync_source: axis.sync_source,
        }
    }
}
