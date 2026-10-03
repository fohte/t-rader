//! `query_media` tool の inner method 実装。
//!
//! 動画/音声 URL を `agent_graph.tool_models` で指定されたモデルに渡し、prompt の指示に
//! 沿ったテキスト応答を返す。discover フェーズがテキストにしか無い材料にアクセス
//! できるようにするための tool。

use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;

use core_application::llm_client::{ChatMessage, ContentPart, FilePart};

use super::dto::{QueryMediaParams, QueryMediaResult};
use super::{StrategyServer, internal_error, invalid_params, litellm_error_to_mcp};

pub(super) const TOOL_NAME: &str = "query_media";

impl StrategyServer {
    pub(crate) async fn query_media_inner(
        &self,
        scope: impl Into<StrategyScope>,
        model: String,
        params: QueryMediaParams,
    ) -> Result<QueryMediaResult, McpError> {
        let session_strategy_id = scope.into().id();
        let media_url = params.media_url.trim().to_string();
        if media_url.is_empty() {
            return Err(invalid_params("media_url must not be empty"));
        }
        let prompt = params.prompt.trim().to_string();
        if prompt.is_empty() {
            return Err(invalid_params("prompt must not be empty"));
        }

        let client = self
            .dependencies
            .llm_client
            .as_ref()
            .ok_or_else(|| internal_error("litellm client is not configured"))?;

        let messages = vec![ChatMessage {
            role: "user",
            content: vec![
                ContentPart::Text { text: prompt },
                ContentPart::File {
                    file: FilePart {
                        file_id: media_url.clone(),
                    },
                },
            ],
        }];

        tracing::info!(
            strategy_id = %session_strategy_id,
            model,
            media_url,
            "query_media: dispatching chat completion request"
        );

        let text = client
            .chat_completion(&model, messages)
            .await
            .map_err(|e| {
                tracing::warn!(
                    strategy_id = %session_strategy_id,
                    model,
                    media_url,
                    error = %e,
                    "query_media: chat completion request failed"
                );
                litellm_error_to_mcp(e)
            })?;

        Ok(QueryMediaResult { text })
    }
}
