use std::sync::Arc;

use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum LlmClientError {
    #[error("network error: {0}")]
    Network(String),

    #[error("request timed out: {0}")]
    Timeout(String),

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

/// Chat completion に対する tool 固有の追加指定。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatCompletionOptions {
    pub max_completion_tokens: Option<u32>,
    pub reasoning_effort: Option<String>,
}

/// Chat completion の本文と provider が返す実行情報。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatCompletionOutput {
    pub content: Option<String>,
    pub finish_reason: Option<String>,
    pub completion_tokens: Option<u64>,
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
    /// URL から形式を判別できない場合に provider が入力形式を特定するためのヒント。
    pub mime_type: Option<String>,
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn list_models(&self) -> Result<Vec<LlmModel>, LlmClientError>;

    async fn chat_completion(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<String, LlmClientError>;

    /// 既存 client は従来の呼び出しに委譲でき、対応 client は追加 option を送信する。
    async fn chat_completion_with_options(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        _options: ChatCompletionOptions,
    ) -> Result<ChatCompletionOutput, LlmClientError> {
        self.chat_completion(model, messages)
            .await
            .map(|content| ChatCompletionOutput {
                content: Some(content),
                finish_reason: None,
                completion_tokens: None,
            })
    }
}

pub type SharedLlmClient = Arc<dyn LlmClient + Send + Sync>;
