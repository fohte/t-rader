use axum::http::{HeaderMap, Request};
use rmcp::ErrorData as McpError;
use rmcp::ServerHandler;
use rmcp::model::{CallToolRequestParams, CallToolResponse, NumberOrString};
use rmcp::service::{RequestContext, RoleServer, serve_directly};
use serde_json::Value;

pub(crate) async fn call_tool<S>(
    server: &S,
    name: &'static str,
    arguments: Value,
    headers: HeaderMap,
) -> Result<Value, McpError>
where
    S: ServerHandler + Clone + 'static,
{
    let arguments = arguments
        .as_object()
        .cloned()
        .ok_or_else(|| McpError::invalid_params("tool arguments must be a JSON object", None))?;

    let (server_io, _client_io) = tokio::io::duplex(64);
    let (reader, writer) = tokio::io::split(server_io);
    let running = serve_directly::<RoleServer, _, _, std::io::Error, _>(
        server.clone(),
        (reader, writer),
        None,
    );

    let mut parts = Request::new(()).into_parts().0;
    parts.headers = headers;
    let mut context = RequestContext::new(NumberOrString::Number(1), running.peer().clone());
    context.extensions.insert(parts);
    let response = server
        .call_tool(
            CallToolRequestParams::new(name).with_arguments(arguments),
            context,
        )
        .await;
    let _ = running.cancel().await;

    let response = response?;
    let response = match response {
        CallToolResponse::Complete(response) => response,
        CallToolResponse::InputRequired(_) | CallToolResponse::Task(_) => {
            return Err(McpError::internal_error(
                "test tool call did not return a complete result",
                None,
            ));
        }
        _ => {
            return Err(McpError::internal_error(
                "test tool call returned an unsupported response",
                None,
            ));
        }
    };
    response
        .structured_content
        .ok_or_else(|| McpError::internal_error("tool result has no structured JSON output", None))
}
