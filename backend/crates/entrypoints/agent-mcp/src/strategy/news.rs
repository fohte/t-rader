//! 戦略実行 MCP のニュース検索 tool 実装。

use core_application::news::{NewsArticle, NewsArticleContent, SearchNewsQuery};
use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;

use super::dto::{
    GetNewsContentParams, GetNewsContentResult, NewsItemDto, SearchNewsParams, SearchNewsResult,
};
use super::{StrategyServer, internal_error};

impl StrategyServer {
    pub(crate) async fn get_news_content_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: GetNewsContentParams,
    ) -> Result<GetNewsContentResult, McpError> {
        let content = self
            .dependencies
            .news
            .get_news_content(scope.into(), params.id)
            .await
            .map_err(map_news_error)?;

        content
            .map(Into::into)
            .ok_or_else(|| McpError::resource_not_found("news item not found", None))
    }

    /// news_item を title/body_snippet/保存済み本文のキーワードと published_at の期間で検索する。
    pub(crate) async fn search_news_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: SearchNewsParams,
    ) -> Result<SearchNewsResult, McpError> {
        let scope = scope.into();
        let rows = self
            .dependencies
            .news
            .search_news(
                scope,
                SearchNewsQuery {
                    keyword: params.keyword,
                    from: params.from,
                    to: params.to,
                    limit: params.limit,
                },
            )
            .await
            .map_err(map_news_error)?;

        Ok(SearchNewsResult {
            items: rows.into_iter().map(Into::into).collect(),
        })
    }
}

impl From<NewsArticle> for NewsItemDto {
    fn from(article: NewsArticle) -> Self {
        Self {
            id: article.id,
            source: article.source,
            url: article.url,
            title: article.title,
            body_snippet: article.body_snippet,
            content_status: article.content_status,
            published_at: article.published_at,
        }
    }
}

impl From<NewsArticleContent> for GetNewsContentResult {
    fn from(content: NewsArticleContent) -> Self {
        Self {
            id: content.id,
            source: content.source,
            url: content.url,
            title: content.title,
            published_at: content.published_at,
            content_status: content.content_status,
            content: content.content,
            content_error: content.content_error,
        }
    }
}

fn map_news_error(error: core_application::news::NewsUseCaseError) -> McpError {
    tracing::error!(error = %error, "strategy mcp news request failed");
    internal_error(format!("database error: {error}"))
}
