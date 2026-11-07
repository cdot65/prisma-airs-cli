//! M0 skeleton of `airs-mcp-client`: proves rmcp's Streamable HTTP client links into
//! the CLI graph on the pinned reqwest without SQLite or server code.

use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, ClientConfig},
    transport::StreamableHttpClientTransport,
    transport::streamable_http_client::StreamableHttpClientTransportConfig,
};

/// Upper bound on one tool result the CLI accepts (contract §5: 256 KiB).
pub const MAX_TOOL_RESULT_BYTES: usize = 256 * 1024;

#[derive(Debug)]
pub enum ClientError {
    Connect,
    Call,
    Oversized,
}

/// Connect, list tools and call one tool with a bearer token. Returns the
/// serialized result, refusing anything larger than [`MAX_TOOL_RESULT_BYTES`].
pub async fn call_tool(
    url: &str,
    bearer: Option<String>,
    tool: &str,
    arguments: serde_json::Map<String, serde_json::Value>,
) -> Result<(Vec<String>, String), ClientError> {
    let mut config = StreamableHttpClientTransportConfig::with_uri(url.to_owned());
    if let Some(token) = bearer {
        config = config.auth_header(token);
    }
    let transport = StreamableHttpClientTransport::from_config(config);
    let client = ClientConfig::default()
        .serve(transport)
        .await
        .map_err(|_| ClientError::Connect)?;
    let tools = client
        .list_all_tools()
        .await
        .map_err(|_| ClientError::Call)?
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();
    let result = client
        .call_tool(CallToolRequestParams::new(tool.to_owned()).with_arguments(arguments))
        .await
        .map_err(|_| ClientError::Call)?;
    let body = serde_json::to_string(&result).map_err(|_| ClientError::Call)?;
    let _ = client.cancel().await;
    if body.len() > MAX_TOOL_RESULT_BYTES {
        return Err(ClientError::Oversized);
    }
    Ok((tools, body))
}

pub fn core_sdk_pin() -> &'static str {
    airs_runtime_core::SDK_VERSION
}
