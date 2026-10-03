use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) const DEFAULT_ANNOTATION_STATUS: &str = "unread";
pub(crate) const STRATEGY_AGENT_ACTOR: &str = "llm";
pub(crate) const MAX_QUERY_DATA_INSTRUMENTS: usize = 100;
pub(crate) const MAX_LIST_LIMIT: u64 = 200;
pub(crate) const SEARCH_WEB_MAX_CALLS_PER_TASK: u32 = 20;
pub(crate) const EXEC_MAX_OUTPUT_BYTES: u32 = 1024 * 1024;
pub(crate) const EXEC_MAX_TIMEOUT_SECS: u32 = 60;
pub(crate) const MAX_CODE_BYTES: usize = 64 * 1024;

pub(crate) mod refs {
    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct SearchRefsParams {
        pub query: String,
        pub limit: Option<u32>,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct RefDto {
        pub ref_kind: String,
        pub ref_id: String,
        pub name: String,
        pub product_category: Option<String>,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct SearchRefsResult {
        pub refs: Vec<RefDto>,
    }
}

pub(crate) mod ref_terms {
    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct AddRefTermsParams {
        pub ref_kind: String,
        pub ref_id: String,
        pub terms: Vec<String>,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct AddRefTermsResult {
        pub added: Vec<String>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct RemoveRefTermsParams {
        pub ref_kind: String,
        pub ref_id: String,
        pub terms: Vec<String>,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct RemoveRefTermsResult {
        pub removed: Vec<String>,
    }
}

pub(crate) mod stock_groups {
    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct CreateStockGroupParams {
        pub axis_key: String,
        pub group_key: String,
        pub name: String,
        pub description: Option<String>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct UpdateStockGroupParams {
        pub axis_key: String,
        pub group_key: String,
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<Option<String>>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct StockGroupMemberParams {
        pub axis_key: String,
        pub group_key: String,
        pub stock_id: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct ListStockGroupMembersParams {
        pub axis_key: String,
        pub group_key: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct StockGroupDto {
        pub id: Uuid,
        pub axis_key: String,
        pub group_key: String,
        pub name: String,
        pub description: Option<String>,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct StockGroupMemberChangeResult {
        pub changed: bool,
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ListStockGroupMembersResult {
        pub axis_key: String,
        pub group_key: String,
        pub stock_ids: Vec<String>,
    }
}
