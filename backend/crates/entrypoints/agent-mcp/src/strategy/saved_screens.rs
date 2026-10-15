use core_application::saved_screen::SavedScreenUseCaseError;
use rmcp::ErrorData as McpError;

use super::dto::{ListSavedScreensResult, SavedScreenDto};
use super::{StrategyServer, internal_error};

impl StrategyServer {
    pub(crate) async fn list_saved_screens_inner(
        &self,
    ) -> Result<ListSavedScreensResult, McpError> {
        let screens = self
            .dependencies
            .saved_screens
            .list()
            .await
            .map_err(saved_screen_error)?
            .into_iter()
            .map(SavedScreenDto::from)
            .collect();
        Ok(ListSavedScreensResult { screens })
    }
}

fn saved_screen_error(error: SavedScreenUseCaseError) -> McpError {
    tracing::error!(error = %error, "strategy mcp saved screens query failed");
    internal_error(format!("database error: {error}"))
}
