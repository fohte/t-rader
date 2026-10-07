use serde::Serialize;
use utoipa::ToSchema;

use core_application::group_axis::GroupAxis;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = GroupAxis)]
pub struct GroupAxisResponse {
    pub key: String,
    pub name: String,
    pub description: String,
    pub derive_from: Option<String>,
}

impl From<GroupAxis> for GroupAxisResponse {
    fn from(axis: GroupAxis) -> Self {
        Self {
            key: axis.key,
            name: axis.name,
            description: axis.description,
            derive_from: axis.derive_from,
        }
    }
}
