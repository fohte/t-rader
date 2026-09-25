use std::sync::Arc;

use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum LlmClientError {
    #[error("network error: {0}")]
    Network(String),

    #[error("LLM API error (status {status}): {message}")]
    Api { status: u16, message: String },

    #[error("failed to parse response: {0}")]
    Parse(String),

    #[error("client initialization error: {0}")]
    Init(String),
}

/// LLM client から取得したモデル情報。
#[derive(Debug, Clone, PartialEq)]
pub struct LlmModel {
    pub id: String,
    pub providers: Vec<String>,
    pub max_input_tokens: Option<f64>,
    pub max_output_tokens: Option<f64>,
    pub supports_reasoning: bool,
}

/// LLM client に送る会話メッセージ。
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: &'static str,
    pub content: Vec<ContentPart>,
}

/// テキストまたは外部ファイルを含むメッセージ内容。
#[derive(Debug, Clone)]
pub enum ContentPart {
    Text { text: String },
    File { file: FilePart },
}

/// メッセージから参照する外部ファイル。
#[derive(Debug, Clone)]
pub struct FilePart {
    pub file_id: String,
}

/// Web 検索の回答と重複を除いた出典 URL。
#[derive(Debug, Clone, PartialEq)]
pub struct WebSearchOutcome {
    pub text: String,
    pub citations: Vec<String>,
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn list_models(&self) -> Result<Vec<LlmModel>, LlmClientError>;

    async fn chat_completion(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<String, LlmClientError>;

    async fn web_search(
        &self,
        model: &str,
        query: &str,
    ) -> Result<WebSearchOutcome, LlmClientError>;
}

pub type SharedLlmClient = Arc<dyn LlmClient + Send + Sync>;
