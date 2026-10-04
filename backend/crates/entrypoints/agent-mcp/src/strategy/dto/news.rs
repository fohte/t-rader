use chrono::{DateTime, FixedOffset, NaiveDate};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SearchNewsParams {
    /// title / body_snippet / 保存済み本文の部分一致 (大文字小文字を区別しない)。省略時はキーワード条件なし
    pub keyword: Option<String>,
    /// 取得開始日 (YYYY-MM-DD, inclusive)
    pub from: Option<NaiveDate>,
    /// 取得終了日 (YYYY-MM-DD, inclusive)
    pub to: Option<NaiveDate>,
    pub limit: Option<u32>,
}

/// `search_news` で返す記事 1 件
#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct NewsItemDto {
    pub id: Uuid,
    pub source: String,
    pub url: String,
    pub title: String,
    pub body_snippet: Option<String>,
    pub content_status: Option<String>,
    pub published_at: DateTime<FixedOffset>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct SearchNewsResult {
    pub items: Vec<NewsItemDto>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetNewsContentParams {
    /// `search_news` が返す記事 ID
    pub id: Uuid,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct GetNewsContentResult {
    pub id: Uuid,
    pub source: String,
    pub url: String,
    pub title: String,
    pub published_at: DateTime<FixedOffset>,
    /// 本文行がない場合は null。値は保存された状態文字列をそのまま返す
    pub content_status: Option<String>,
    pub content: Option<String>,
    pub content_error: Option<String>,
}
