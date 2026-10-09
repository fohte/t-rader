use serde_json::{Value, json};

pub fn legacy_initialize_body() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "test-client", "version": "0.0.0" },
        },
    })
}

pub fn parse_sse_response(body: &str) -> Value {
    body.lines()
        .filter_map(|line| line.strip_prefix("data:").map(str::trim))
        .find_map(|payload| serde_json::from_str(payload).ok())
        .expect("no JSON data line in MCP response")
}
