//! 戦略実行 MCP のニュース検索 tool 実装。

use core_application::news::{NewsArticle, SearchNewsQuery};
use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;

use super::dto::{NewsItemDto, SearchNewsParams, SearchNewsResult};
use super::{StrategyServer, internal_error};

impl StrategyServer {
    /// news_item を title/body_snippet のキーワードと published_at の期間で検索する。
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
            published_at: article.published_at,
        }
    }
}

fn map_news_error(error: core_application::news::NewsUseCaseError) -> McpError {
    tracing::error!(error = %error, "strategy mcp news search failed");
    internal_error(format!("database error: {error}"))
}
