use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap};
use axum::response::{Html, IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::AdapterError;
use crate::http::AppState;
use crate::protocol::{parse_tool_calls, render_tool_protocol_prompt};
use crate::responses_store;
use crate::types::{ParsedToolCall, UnifiedContent, UnifiedMessage, UnifiedRequest};
use crate::ui::INDEX_HTML;
use crate::upstream::UpstreamRequestOptions;
use crate::wire::{chat, messages, responses, WireMode};

const MAX_AUTO_TOOL_ROUNDS: usize = 4;

pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

pub async fn ui() -> Html<&'static str> {
    Html(INDEX_HTML)
}

pub async fn deepseek_web_login(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, AdapterError> {
    verify_auth(&state, &headers)?;
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| AdapterError::Upstream("cannot resolve workspace root".to_string()))?;
    let cli_manifest = workspace_root
        .join("crates")
        .join("aeon-claw-cli")
        .join("Cargo.toml");
    if !cli_manifest.exists() {
        return Err(AdapterError::Upstream(format!(
            "FCACoreai Rust DeepSeek login CLI not found: {}",
            cli_manifest.display()
        )));
    }
    let log_path = deepseek_login_log_path()?;
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            AdapterError::Upstream(format!(
                "failed to create DeepSeek login log dir {}: {error}",
                parent.display()
            ))
        })?;
    }
    let stdout = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|error| {
            AdapterError::Upstream(format!(
                "failed to open DeepSeek login log {}: {error}",
                log_path.display()
            ))
        })?;
    let stderr = stdout.try_clone().map_err(|error| {
        AdapterError::Upstream(format!(
            "failed to clone DeepSeek login log {}: {error}",
            log_path.display()
        ))
    })?;

    std::process::Command::new("cargo")
        .args([
            "run",
            "-p",
            "aeon-claw-cli",
            "--bin",
            "aeon-claw-cli",
            "--",
            "tool",
            "DeepSeekLogin",
            "{}",
        ])
        .current_dir(workspace_root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(stdout))
        .stderr(std::process::Stdio::from(stderr))
        .spawn()
        .map_err(|error| {
            AdapterError::Upstream(format!(
                "failed to start FCACoreai Rust DeepSeek login: {error}"
            ))
        })?;

    Ok(Json(json!({
        "status": "started",
        "message": "FCACoreai Rust DeepSeekLogin started. Finish login, then fetch models again.",
        "session_file": "~/.FCACore/deepseek_session.json",
        "log_file": log_path,
        "command": "cargo run -p aeon-claw-cli --bin aeon-claw-cli -- tool DeepSeekLogin {}",
    })))
}

pub async fn market_overview(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, AdapterError> {
    verify_auth(&state, &headers)?;
    let params = parse_market_overview_params(&payload)?;

    let result = state
        .upstream
        .fetch_market_overview(
            &params.base_url,
            &params.symbol,
            &params.market,
            &params.modules,
        )
        .await?;
    Ok(Json(result))
}

struct MarketOverviewParams {
    symbol: String,
    market: String,
    modules: Vec<String>,
    base_url: String,
}

fn parse_market_overview_params(payload: &Value) -> Result<MarketOverviewParams, AdapterError> {
    let symbol = payload
        .get("symbol")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AdapterError::InvalidRequest("symbol is required".to_string()))?
        .to_string();
    let market = payload
        .get("market")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("hk")
        .to_string();
    let modules = payload
        .get("modules")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| vec!["price".to_string()]);
    let base_url = payload
        .get("base_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("http://47.238.165.205:8009/overview")
        .to_string();

    Ok(MarketOverviewParams {
        symbol,
        market,
        modules,
        base_url,
    })
}

fn deepseek_login_log_path() -> Result<std::path::PathBuf, AdapterError> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| AdapterError::Upstream("HOME is not set".to_string()))?;
    Ok(std::path::PathBuf::from(home)
        .join(".FCACore")
        .join("deepseek_login_adapter.log"))
}

pub async fn models(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, AdapterError> {
    verify_auth(&state, &headers)?;
    let upstream_options = upstream_options_from_headers(&headers);
    Ok(Json(
        state
            .upstream
            .list_models(
                &state.config.upstream_model,
                &state.config.model_alias_map(),
                &upstream_options,
            )
            .await?,
    ))
}

pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, AdapterError> {
    tracing::info!("Route chat_completions: headers={:?}, payload={:?}", headers, payload);
    verify_auth(&state, &headers)?;
    handle(
        state,
        payload,
        WireMode::ChatCompletions,
        upstream_options_from_headers(&headers),
    )
    .await
}

pub async fn messages(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, AdapterError> {
    verify_auth(&state, &headers)?;
    handle(
        state,
        payload,
        WireMode::Messages,
        upstream_options_from_headers(&headers),
    )
    .await
}

pub async fn responses(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(mut payload): Json<Value>,
) -> Result<Response, AdapterError> {
    tracing::info!("Route responses: headers={:?}, payload={:?}", headers, payload);
    verify_auth(&state, &headers)?;
    let stream = payload
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if stream {
        payload["stream"] = Value::Bool(false);
    }
    let body = handle_value(
        state,
        payload,
        WireMode::Responses,
        upstream_options_from_headers(&headers),
    )
    .await?;
    if stream {
        Ok(responses_sse(body))
    } else {
        Ok(Json(body).into_response())
    }
}

pub async fn responses_retrieve(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(response_id): Path<String>,
) -> Result<Json<Value>, AdapterError> {
    verify_auth(&state, &headers)?;
    let body = state
        .responses
        .retrieve(&response_id)
        .ok_or_else(|| AdapterError::NotFound(format!("response {response_id} not found")))?;
    Ok(Json(body))
}

pub async fn responses_input_items(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(response_id): Path<String>,
    Query(query): Query<ResponseItemsQuery>,
) -> Result<Json<Value>, AdapterError> {
    verify_auth(&state, &headers)?;
    let ascending = query
        .order
        .as_deref()
        .map(|order| order.eq_ignore_ascii_case("asc"))
        .unwrap_or(false);
    let body = state
        .responses
        .list_input_items(&response_id, query.after.as_deref(), query.limit, ascending)
        .ok_or_else(|| AdapterError::NotFound(format!("response {response_id} not found")))?;
    Ok(Json(body))
}

pub async fn responses_cancel(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(response_id): Path<String>,
) -> Result<Json<Value>, AdapterError> {
    verify_auth(&state, &headers)?;
    let body = match state.responses.cancel(&response_id) {
        Ok(body) => body,
        Err(crate::responses_store::StoreError::NotFound) => {
            return Err(AdapterError::NotFound(format!(
                "response {response_id} not found"
            )))
        }
        Err(crate::responses_store::StoreError::NotBackground) => {
            return Err(AdapterError::InvalidRequest(
                "only background responses can be cancelled".to_string(),
            ))
        }
    };
    Ok(Json(body))
}

pub async fn responses_compact(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, AdapterError> {
    verify_auth(&state, &headers)?;
    let request = if payload.get("input").is_some() {
        UnifiedRequest::from_wire_payload(WireMode::Responses, payload.clone())?
    } else {
        UnifiedRequest {
            model: payload
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            max_tokens: payload
                .get("max_output_tokens")
                .or_else(|| payload.get("max_tokens"))
                .and_then(Value::as_u64)
                .unwrap_or(1024) as u32,
            system: payload
                .get("instructions")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            messages: Vec::new(),
            tools: chat::parse_tools(payload.get("tools")),
            stream: payload
                .get("stream")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            background: false,
            previous_response_id: payload
                .get("previous_response_id")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        }
    };
    let mut output = if let Some(previous_response_id) = request.previous_response_id.as_deref() {
        state
            .responses
            .context_items_for(previous_response_id)
            .ok_or_else(|| {
                AdapterError::NotFound(format!(
                    "previous response {previous_response_id} not found"
                ))
            })?
    } else {
        Vec::new()
    };
    output.extend(responses_store::input_items_from_request(&request));
    Ok(Json(json!({
        "id": format!("cmp_{}", uuid::Uuid::new_v4()),
        "created_at": chrono::Utc::now().timestamp(),
        "object": "response.compaction",
        "output": output,
    })))
}

fn verify_auth(state: &AppState, headers: &HeaderMap) -> Result<(), AdapterError> {
    let expected = state.config.adapter_api_key.trim();
    let bearer = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim);
    let api_key = headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim);
    
    tracing::info!("verify_auth: expected='{}', got bearer={:?}, got x-api-key={:?}", expected, bearer, api_key);
    
    if expected.is_empty() {
        return Ok(());
    }
    if bearer == Some(expected) || api_key == Some(expected) {
        Ok(())
    } else {
        tracing::warn!("verify_auth FAILED: keys do not match");
        Err(AdapterError::Unauthorized)
    }
}

async fn handle(
    state: Arc<AppState>,
    payload: Value,
    mode: WireMode,
    upstream_options: UpstreamRequestOptions,
) -> Result<Json<Value>, AdapterError> {
    Ok(Json(
        handle_value(state, payload, mode, upstream_options).await?,
    ))
}

async fn handle_value(
    state: Arc<AppState>,
    payload: Value,
    mode: WireMode,
    upstream_options: UpstreamRequestOptions,
) -> Result<Value, AdapterError> {
    let mut request = UnifiedRequest::from_wire_payload(mode, payload)?;
    tracing::info!("Received request with model={:?}, upstream_options={:?}", request.model, upstream_options);
    if request.stream {
        return Err(AdapterError::StreamUnsupported);
    }
    let response_input_items = matches!(mode, WireMode::Responses)
        .then(|| responses_store::input_items_from_request(&request));
    if matches!(mode, WireMode::Responses) {
        prepend_previous_response_context(&state, &mut request)?;
    }
    if request.model.trim().is_empty() || request.model.trim() == "local-model" {
        request.model = state.config.upstream_model.clone();
    }
    let external_model = request.model.clone();
    if let Some(upstream_model) = state.config.model_alias_map().get(request.model.trim()) {
        request.model = upstream_model.clone();
    }
    tracing::info!("Mapped request model to={:?}", request.model);
    request.tools.truncate(state.config.max_tool_definitions);

    let protocol = render_tool_protocol_prompt(&request.tools);
    let body = match mode {
        WireMode::Responses => {
            let mut body = create_responses_body_with_auto_tools(
                &state,
                request.clone(),
                &protocol,
                &upstream_options,
            )
            .await?;
            restore_external_model(&mut body, &external_model);
            body
        }
        WireMode::ChatCompletions | WireMode::Messages => {
            let prompt = request.render_prompt_with_tool_protocol(&protocol);
            let model_text = state
                .upstream
                .complete(&request, &prompt, &upstream_options)
                .await?;
            let tool_calls = parse_tool_calls(&model_text);
            let mut body = match mode {
                WireMode::ChatCompletions => chat::response(&request, &model_text, &tool_calls),
                WireMode::Messages => messages::response(&request, &model_text, &tool_calls),
                WireMode::Responses => unreachable!(),
            };
            restore_external_model(&mut body, &external_model);
            body
        }
    };
    if matches!(mode, WireMode::Responses) {
        let context_items = responses_store::input_items_from_request(&request);
        state.responses.insert(
            body.clone(),
            response_input_items.unwrap_or_else(|| context_items.clone()),
            context_items,
            request.background,
        );
    }
    Ok(body)
}

fn responses_sse(body: Value) -> Response {
    let mut stream = String::new();
    let mut in_progress = body.clone();
    in_progress["status"] = Value::String("in_progress".to_string());
    in_progress["output"] = json!([]);

    push_sse(
        &mut stream,
        "response.created",
        json!({ "type": "response.created", "response": in_progress }),
    );
    push_sse(
        &mut stream,
        "response.in_progress",
        json!({ "type": "response.in_progress", "response": body_without_output(&body) }),
    );

    for (index, item) in body
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        push_sse(
            &mut stream,
            "response.output_item.added",
            json!({
                "type": "response.output_item.added",
                "output_index": index,
                "item": item,
            }),
        );
        push_message_text_events(&mut stream, index, item);
        push_sse(
            &mut stream,
            "response.output_item.done",
            json!({
                "type": "response.output_item.done",
                "output_index": index,
                "item": item,
            }),
        );
    }

    push_sse(
        &mut stream,
        "response.completed",
        json!({ "type": "response.completed", "response": body }),
    );
    stream.push_str("data: [DONE]\n\n");

    ([(header::CONTENT_TYPE, "text/event-stream")], stream).into_response()
}

fn body_without_output(body: &Value) -> Value {
    let mut value = body.clone();
    value["output"] = json!([]);
    value
}

fn push_message_text_events(stream: &mut String, output_index: usize, item: &Value) {
    if item.get("type").and_then(Value::as_str) != Some("message") {
        return;
    }
    let Some(content) = item.get("content").and_then(Value::as_array) else {
        return;
    };
    for (content_index, part) in content.iter().enumerate() {
        if part.get("type").and_then(Value::as_str) != Some("output_text") {
            continue;
        }
        let text = part.get("text").and_then(Value::as_str).unwrap_or_default();
        push_sse(
            stream,
            "response.content_part.added",
            json!({
                "type": "response.content_part.added",
                "output_index": output_index,
                "content_index": content_index,
                "part": part,
            }),
        );
        push_sse(
            stream,
            "response.output_text.delta",
            json!({
                "type": "response.output_text.delta",
                "output_index": output_index,
                "content_index": content_index,
                "delta": text,
            }),
        );
        push_sse(
            stream,
            "response.output_text.done",
            json!({
                "type": "response.output_text.done",
                "output_index": output_index,
                "content_index": content_index,
                "text": text,
            }),
        );
        push_sse(
            stream,
            "response.content_part.done",
            json!({
                "type": "response.content_part.done",
                "output_index": output_index,
                "content_index": content_index,
                "part": part,
            }),
        );
    }
}

fn push_sse(stream: &mut String, event: &str, data: Value) {
    stream.push_str("event: ");
    stream.push_str(event);
    stream.push('\n');
    stream.push_str("data: ");
    stream.push_str(&data.to_string());
    stream.push_str("\n\n");
}

fn restore_external_model(body: &mut Value, external_model: &str) {
    if !external_model.trim().is_empty() {
        body["model"] = Value::String(external_model.to_string());
    }
}

async fn create_responses_body_with_auto_tools(
    state: &AppState,
    mut request: UnifiedRequest,
    protocol: &str,
    upstream_options: &UpstreamRequestOptions,
) -> Result<Value, AdapterError> {
    let mut output = Vec::new();
    for _ in 0..MAX_AUTO_TOOL_ROUNDS {
        let prompt = request.render_prompt_with_tool_protocol(protocol);
        let model_text = state
            .upstream
            .complete(&request, &prompt, upstream_options)
            .await?;
        let tool_calls = parse_tool_calls(&model_text);
        if tool_calls.is_empty() {
            output.push(responses::message_output_item(&model_text));
            return Ok(responses::response_from_output(
                &request,
                output,
                &model_text,
            ));
        }

        if !tool_calls.iter().all(|call| is_internal_tool(&call.name)) {
            output.extend(responses::function_call_items(&tool_calls));
            return Ok(responses::response_from_output(&request, output, ""));
        }

        output.extend(responses::function_call_items(&tool_calls));
        for (call, tool_output) in execute_internal_tool_calls_parallel(state, &tool_calls).await? {
            output.push(responses::function_call_output_item(&call.id, &tool_output));
            request.messages.push(UnifiedMessage {
                role: "assistant".to_string(),
                content: vec![UnifiedContent::ToolUse {
                    id: call.id.clone(),
                    name: call.name.clone(),
                    input: serde_json::from_str::<Value>(&call.arguments)
                        .unwrap_or_else(|_| json!({})),
                }],
            });
            request.messages.push(UnifiedMessage {
                role: "user".to_string(),
                content: vec![UnifiedContent::ToolResult {
                    tool_use_id: call.id.clone(),
                    content: tool_output,
                    is_error: false,
                }],
            });
        }
    }

    Err(AdapterError::InvalidRequest(format!(
        "auto tool call limit exceeded after {MAX_AUTO_TOOL_ROUNDS} rounds"
    )))
}

fn is_internal_tool(name: &str) -> bool {
    matches!(name.trim(), "get_overview" | "get_quote")
}

async fn execute_internal_tool_calls_parallel(
    state: &AppState,
    calls: &[ParsedToolCall],
) -> Result<Vec<(ParsedToolCall, String)>, AdapterError> {
    let mut handles = Vec::with_capacity(calls.len());
    for (index, call) in calls.iter().cloned().enumerate() {
        let state = state.clone();
        handles.push(tokio::spawn(async move {
            let output = execute_internal_tool_call(&state, &call).await?;
            Ok::<_, AdapterError>((index, call, output))
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        let result = handle
            .await
            .map_err(|error| AdapterError::Upstream(format!("tool task join error: {error}")))??;
        results.push(result);
    }
    results.sort_by_key(|(index, _, _)| *index);
    Ok(results
        .into_iter()
        .map(|(_, call, output)| (call, output))
        .collect())
}

async fn execute_internal_tool_call(
    state: &AppState,
    call: &ParsedToolCall,
) -> Result<String, AdapterError> {
    let args = serde_json::from_str::<Value>(&call.arguments).map_err(|error| {
        AdapterError::InvalidRequest(format!(
            "tool {} arguments must be valid JSON: {error}",
            call.name
        ))
    })?;
    let params = parse_market_overview_params(&args)?;
    let result = state
        .upstream
        .fetch_market_overview(
            &params.base_url,
            &params.symbol,
            &params.market,
            &params.modules,
        )
        .await?;
    serde_json::to_string(&result)
        .map_err(|error| AdapterError::Upstream(format!("serialize tool result: {error}")))
}

fn prepend_previous_response_context(
    state: &AppState,
    request: &mut UnifiedRequest,
) -> Result<(), AdapterError> {
    let Some(previous_response_id) = request.previous_response_id.as_deref() else {
        return Ok(());
    };
    let previous_items = state
        .responses
        .context_items_for(previous_response_id)
        .ok_or_else(|| {
            AdapterError::NotFound(format!(
                "previous response {previous_response_id} not found"
            ))
        })?;
    let mut previous_messages = responses::parse_input(&Value::Array(previous_items));
    previous_messages.append(&mut request.messages);
    request.messages = previous_messages;
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseItemsQuery {
    pub after: Option<String>,
    pub limit: Option<usize>,
    pub order: Option<String>,
}

fn upstream_options_from_headers(headers: &HeaderMap) -> UpstreamRequestOptions {
    UpstreamRequestOptions {
        provider: header_string(headers, "x-upstream-provider"),
        base_url: header_string(headers, "x-upstream-base-url"),
        api_key: header_string(headers, "x-upstream-api-key"),
        deepseek_session: header_string(headers, "x-deepseek-session"),
    }
}

fn header_string(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderMap;

    use super::upstream_options_from_headers;

    #[test]
    fn extracts_per_request_upstream_options() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-upstream-base-url",
            "https://api.example.com/v1".parse().unwrap(),
        );
        headers.insert("x-upstream-api-key", "secret".parse().unwrap());
        headers.insert("x-upstream-provider", "deepseek-web".parse().unwrap());
        headers.insert("x-deepseek-session", "cookie=a".parse().unwrap());

        let options = upstream_options_from_headers(&headers);

        assert_eq!(options.provider.as_deref(), Some("deepseek-web"));
        assert_eq!(
            options.base_url.as_deref(),
            Some("https://api.example.com/v1")
        );
        assert_eq!(options.api_key.as_deref(), Some("secret"));
        assert_eq!(options.deepseek_session.as_deref(), Some("cookie=a"));
    }
}
