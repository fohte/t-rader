use axum::http::{HeaderMap, Request};
use rmcp::ErrorData as McpError;
use rmcp::ServerHandler;
use rmcp::model::{CallToolRequestParams, CallToolResponse, NumberOrString};
use rmcp::service::{RequestContext, RoleServer, serve_directly};
use serde::de::DeserializeOwned;
use serde_json::Value;

pub(crate) async fn call_tool<S>(
    server: &S,
    name: &'static str,
    arguments: Value,
) -> Result<CallToolResponse, McpError>
where
    S: ServerHandler + Clone + 'static,
{
    call_tool_with_headers(server, name, arguments, HeaderMap::new()).await
}

pub(crate) async fn call_tool_with_headers<S>(
    server: &S,
    name: &'static str,
    arguments: Value,
    headers: HeaderMap,
) -> Result<CallToolResponse, McpError>
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
    response
}

pub(crate) async fn call_tool_output<S, T>(
    server: &S,
    name: &'static str,
    arguments: Value,
) -> Result<T, McpError>
where
    S: ServerHandler + Clone + 'static,
    T: DeserializeOwned,
{
    let response = call_tool(server, name, arguments).await?;
    decode_tool_output(response)
}

pub(crate) async fn call_tool_output_with_headers<S, T>(
    server: &S,
    name: &'static str,
    arguments: Value,
    headers: HeaderMap,
) -> Result<T, McpError>
where
    S: ServerHandler + Clone + 'static,
    T: DeserializeOwned,
{
    let response = call_tool_with_headers(server, name, arguments, headers).await?;
    decode_tool_output(response)
}

fn decode_tool_output<T: DeserializeOwned>(response: CallToolResponse) -> Result<T, McpError> {
    let CallToolResponse::Complete(response) = response else {
        return Err(McpError::internal_error(
            "test tool call did not return a complete result",
            None,
        ));
    };
    if response.is_error == Some(true) {
        return Err(McpError::internal_error(
            "test tool call returned an error result",
            None,
        ));
    }
    let structured_content = response.structured_content.ok_or_else(|| {
        McpError::internal_error("tool result has no structured JSON output", None)
    })?;
    serde_json::from_value(structured_content)
        .map_err(|error| McpError::internal_error(error.to_string(), None))
}
