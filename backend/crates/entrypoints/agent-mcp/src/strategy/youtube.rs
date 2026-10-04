//! `query_youtube` tool の inner method 実装。
//!
//! YouTube 動画 URL と複数の質問を `agent_graph.tool_models` で指定されたモデルに渡し、
//! 回答を返す。

use axum::http::Uri;
use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;

use core_application::llm_client::{ChatMessage, ContentPart, FilePart};

use super::dto::{QueryYoutubeParams, QueryYoutubeResult};
use super::{StrategyServer, internal_error, invalid_params, litellm_error_to_mcp};

pub(super) const TOOL_NAME: &str = "query_youtube";

impl StrategyServer {
    pub(crate) async fn query_youtube_inner(
        &self,
        scope: impl Into<StrategyScope>,
        model: String,
        params: QueryYoutubeParams,
    ) -> Result<QueryYoutubeResult, McpError> {
        let session_strategy_id = scope.into().id();
        let youtube_url = params.youtube_url.trim().to_string();
        if youtube_url.is_empty() {
            return Err(invalid_params("youtube_url must not be empty"));
        }

        let uri = youtube_url
            .parse::<Uri>()
            .map_err(|_| invalid_params("youtube_url must be a valid YouTube URL"))?;
        let host = uri.host().unwrap_or_default();
        if ![
            "youtube.com",
            "www.youtube.com",
            "m.youtube.com",
            "youtu.be",
        ]
        .iter()
        .any(|allowed| host.eq_ignore_ascii_case(allowed))
        {
            return Err(invalid_params(
                "youtube_url must use an allowed YouTube host",
            ));
        }

        if params.questions.is_empty() {
            return Err(invalid_params("questions must not be empty"));
        }
        let questions = params
            .questions
            .into_iter()
            .map(|question| question.trim().to_string())
            .collect::<Vec<_>>();
        if questions.iter().any(String::is_empty) {
            return Err(invalid_params("questions must not contain empty values"));
        }

        let numbered_questions = questions
            .iter()
            .enumerate()
            .map(|(index, question)| format!("{}. {question}", index + 1))
            .collect::<Vec<_>>()
            .join("; ");
        let prompt = format!(
            "Answer each question about this video. Include an MM:SS timestamp whenever you mention a numerical value or attribute a statement to the video. Questions: {numbered_questions}"
        );

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
                        file_id: youtube_url.clone(),
                        mime_type: Some("video/mp4".to_string()),
                    },
                },
            ],
        }];

        tracing::info!(
            strategy_id = %session_strategy_id,
            model,
            youtube_url,
            "query_youtube: dispatching chat completion request"
        );

        let text = client
            .chat_completion(&model, messages)
            .await
            .map_err(|e| {
                tracing::warn!(
                    strategy_id = %session_strategy_id,
                    model,
                    youtube_url,
                    error = %e,
                    "query_youtube: chat completion request failed"
                );
                litellm_error_to_mcp(e)
            })?;

        Ok(QueryYoutubeResult { text })
    }
}
