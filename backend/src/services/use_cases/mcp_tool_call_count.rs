use std::sync::Arc;

use core_application::mcp_tool_call_count::McpToolCallCountUseCases;
use gateway_postgres::PostgresMcpToolCallCountRepository;

use super::UseCases;

impl UseCases {
    pub fn mcp_tool_call_counts(&self) -> McpToolCallCountUseCases {
        McpToolCallCountUseCases::new(Arc::new(PostgresMcpToolCallCountRepository::new(
            self.db.clone(),
        )))
    }
}
