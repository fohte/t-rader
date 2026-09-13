use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::entities::{hypothesis, hypothesis_proposal};

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewHypothesisProposalRequest {
    /// 任意のレビューコメント
    pub review_note: Option<String>,
}

/// 提案承認のレスポンス。承認により反映された仮説本体を併せて返す。
#[derive(Debug, Serialize, ToSchema)]
pub struct ApproveHypothesisProposalResponse {
    pub proposal: hypothesis_proposal::Model,
    pub hypothesis: hypothesis::Model,
}
