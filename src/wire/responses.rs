use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::AdapterError;
use crate::types::{
    ParsedToolCall, ToolDefinition, UnifiedContent, UnifiedMessage, UnifiedRequest,
};
use crate::wire::chat;

pub fn parse_request(payload: Value) -> Result<UnifiedRequest, AdapterError> {
    let model = payload
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let max_tokens = payload
        .get("max_output_tokens")
        .or_else(|| payload.get("max_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(1024) as u32;
    let stream = payload
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let background = payload
        .get("background")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let system = payload
        .get("instructions")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let previous_response_id = payload
        .get("previous_response_id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let tool_choice = chat::parse_tool_choice(payload.get("tool_choice"));
    let tools = chat::parse_tools(payload.get("tools"));
    chat::reject_hosted_only_tools(payload.get("tools"), &tools, &tool_choice)?;
    let parallel_tool_calls = payload
        .get("parallel_tool_calls")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let messages = payload.get("input").map(parse_input).unwrap_or_default();

    Ok(UnifiedRequest {
        model,
        max_tokens,
        system,
        messages,
        tools,
        tool_choice,
        parallel_tool_calls,
        stream,
        background,
        previous_response_id,
    })
}

#[cfg(test)]
fn response(request: &UnifiedRequest, model_text: &str, tool_calls: &[ParsedToolCall]) -> Value {
    let output = if tool_calls.is_empty() {
        vec![message_output_item(model_text)]
    } else {
        function_call_items(tool_calls)
    };
    response_from_output(
        request,
        output,
        if tool_calls.is_empty() {
            model_text
        } else {
            ""
        },
    )
}

pub(crate) fn response_from_output(
    request: &UnifiedRequest,
    output: Vec<Value>,
    output_text: &str,
) -> Value {
    json!({
        "id": format!("resp_{}", Uuid::new_v4()),
        "object": "response",
        "created_at": Utc::now().timestamp(),
        "completed_at": Utc::now().timestamp(),
        "status": "completed",
        "error": null,
        "incomplete_details": null,
        "instructions": request.system,
        "max_output_tokens": request.max_tokens,
        "model": request.model,
        "output": output,
        "output_text": output_text,
        "parallel_tool_calls": request.parallel_tool_calls,
        "previous_response_id": request.previous_response_id,
        "store": true,
        "tool_choice": request.tool_choice.to_wire_value(),
        "tools": response_tools(&request.tools),
        "truncation": "disabled",
        "usage": null
    })
}

pub(crate) fn message_output_item(text: &str) -> Value {
    json!({
        "id": format!("msg_{}", Uuid::new_v4()),
        "type": "message",
        "status": "completed",
        "role": "assistant",
        "content": [{ "type": "output_text", "text": text, "annotations": [] }]
    })
}

pub(crate) fn reasoning_output_item(reasoning: &str) -> Value {
    json!({
        "id": format!("rs_{}", Uuid::new_v4()),
        "type": "reasoning",
        "status": "completed",
        "summary": [{
            "type": "summary_text",
            "text": reasoning
        }]
    })
}

pub(crate) fn function_call_items(tool_calls: &[ParsedToolCall]) -> Vec<Value> {
    tool_calls
        .iter()
        .map(|call| {
            let mut item = json!({
                "id": format!("fc_{}", Uuid::new_v4()),
                "type": "function_call",
                "status": "completed",
                "call_id": call.id,
                "name": call.name,
                "arguments": call.arguments
            });
            if let Some(namespace) = codex_namespace_from_tool_name(&call.name) {
                item["namespace"] = Value::String(namespace);
            }
            item
        })
        .collect()
}

fn codex_namespace_from_tool_name(name: &str) -> Option<String> {
    name.strip_prefix("mcp__")
        .and_then(|rest| rest.split_once("__"))
        .map(|(namespace, _)| namespace.to_string())
}

fn response_tools(tools: &[ToolDefinition]) -> Value {
    json!(tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.input_schema,
                "strict": false,
            })
        })
        .collect::<Vec<_>>())
}

pub(crate) fn parse_input(input: &Value) -> Vec<UnifiedMessage> {
    match input {
        Value::String(text) => vec![UnifiedMessage::text("user", text.clone())],
        Value::Array(items) => items.iter().filter_map(parse_input_item).collect(),
        other => vec![UnifiedMessage::text("user", other.to_string())],
    }
}

fn parse_input_item(item: &Value) -> Option<UnifiedMessage> {
    match item.get("type").and_then(Value::as_str) {
        Some("compaction_trigger")
        | Some("item_reference")
        | Some("reasoning")
        | Some("compaction") => None,
        Some("message") => {
            let role = item.get("role").and_then(Value::as_str).unwrap_or("user");
            Some(UnifiedMessage {
                role: role.to_string(),
                content: parse_message_content(item.get("content")),
            })
        }
        Some("function_call_output") => Some(UnifiedMessage {
            role: "user".to_string(),
            content: vec![UnifiedContent::ToolResult {
                tool_use_id: item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .unwrap_or("call_unknown")
                    .to_string(),
                content: function_call_output_to_text(item.get("output")),
                is_error: false,
            }],
        }),
        Some("function_call") => Some(UnifiedMessage {
            role: "assistant".to_string(),
            content: vec![UnifiedContent::ToolUse {
                id: item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .unwrap_or("call_unknown")
                    .to_string(),
                name: item.get("name").and_then(Value::as_str)?.to_string(),
                input: item
                    .get("arguments")
                    .and_then(Value::as_str)
                    .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                    .or_else(|| item.get("arguments").cloned())
                    .unwrap_or_else(|| json!({})),
            }],
        }),
        _ => {
            let role = item.get("role").and_then(Value::as_str).unwrap_or("user");
            Some(UnifiedMessage {
                role: role.to_string(),
                content: parse_message_content(item.get("content")),
            })
        }
    }
}

fn function_call_output_to_text(output: Option<&Value>) -> String {
    let Some(output) = output else {
        return String::new();
    };
    if let Some(text) = output.as_str() {
        return text.to_string();
    }
    if let Some(parts) = output.as_array() {
        let text_parts = parts
            .iter()
            .filter_map(|part| {
                let part_type = part.get("type").and_then(Value::as_str).unwrap_or_default();
                match part_type {
                    "input_text" | "output_text" | "text" => part
                        .get("text")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned),
                    "input_image" | "image_url" => part
                        .get("image_url")
                        .or_else(|| part.pointer("/image_url/url"))
                        .and_then(Value::as_str)
                        .map(|url| format!("[image: {url}]")),
                    "encrypted_content" => Some("[encrypted_content]".to_string()),
                    _ => Some(part.to_string()),
                }
            })
            .filter(|part| !part.trim().is_empty())
            .collect::<Vec<_>>();
        return text_parts.join("\n");
    }
    output.to_string()
}

fn parse_message_content(value: Option<&Value>) -> Vec<UnifiedContent> {
    match value {
        Some(Value::String(text)) => vec![UnifiedContent::Text { text: text.clone() }],
        Some(Value::Array(parts)) => parts.iter().filter_map(parse_content_part).collect(),
        Some(other) => vec![UnifiedContent::Text {
            text: other.to_string(),
        }],
        None => Vec::new(),
    }
}

fn parse_content_part(part: &Value) -> Option<UnifiedContent> {
    match part
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("input_text")
    {
        "input_text" | "output_text" | "text" => Some(UnifiedContent::Text {
            text: part
                .get("text")
                .or_else(|| part.get("input_text"))
                .and_then(Value::as_str)?
                .to_string(),
        }),
        "input_image" | "image_url" => Some(UnifiedContent::ImageUrl {
            url: part
                .get("image_url")
                .or_else(|| part.pointer("/image_url/url"))
                .and_then(Value::as_str)?
                .to_string(),
        }),
        "input_file" => Some(UnifiedContent::Text {
            text: format!(
                "[File: {}]",
                part.get("filename")
                    .or_else(|| part.get("file_id"))
                    .or_else(|| part.get("file_url"))
                    .and_then(Value::as_str)
                    .unwrap_or("attached file")
            ),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{parse_request, reasoning_output_item, response};
    use crate::types::ParsedToolCall;

    #[test]
    fn parses_responses_function_call_output() {
        let request = parse_request(json!({
            "model": "m",
            "input": [
                {"type":"function_call_output","call_id":"call_a","output":"ok"}
            ]
        }))
        .unwrap();

        assert!(request.messages[0].content_text().contains("call_a"));
    }

    #[test]
    fn parses_responses_function_tool_schema() {
        let request = parse_request(json!({
            "model": "m",
            "input": "hi",
            "tools": [{
                "type": "function",
                "name": "get_weather",
                "description": "Get weather by city",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "city": { "type": "string" }
                    },
                    "required": ["city"]
                },
                "strict": true
            }]
        }))
        .unwrap();

        assert_eq!(request.tools[0].name, "get_weather");
        assert_eq!(request.tools[0].input_schema["required"][0], "city");
    }

    #[test]
    fn parses_responses_message_content_parts() {
        let request = parse_request(json!({
            "model": "m",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [
                    {"type":"input_text","text":"look"},
                    {"type":"input_image","image_url":"https://example.com/a.png"}
                ]
            }]
        }))
        .unwrap();

        assert_eq!(request.messages[0].content.len(), 2);
        assert!(request.messages[0]
            .content_text()
            .contains("https://example.com/a.png"));
    }

    #[test]
    fn permits_previous_response_without_new_input() {
        let request = parse_request(json!({
            "model": "m",
            "previous_response_id": "resp_a"
        }))
        .unwrap();

        assert_eq!(request.previous_response_id.as_deref(), Some("resp_a"));
        assert!(request.messages.is_empty());
    }

    #[test]
    fn emits_responses_function_call() {
        let request = parse_request(json!({
            "model": "m",
            "input": "hi"
        }))
        .unwrap();
        let body = response(
            &request,
            "",
            &[ParsedToolCall {
                id: "call_a".to_string(),
                name: "search".to_string(),
                arguments: r#"{"q":"rs"}"#.to_string(),
            }],
        );

        assert_eq!(body["output"][0]["type"], "function_call");
        assert_eq!(body["output"][0]["status"], "completed");
        assert_eq!(body["output"][0]["call_id"], "call_a");
    }

    #[test]
    fn emits_codex_mcp_namespace_for_function_call() {
        let request = parse_request(json!({
            "model": "m",
            "input": "hi"
        }))
        .unwrap();
        let body = response(
            &request,
            "",
            &[ParsedToolCall {
                id: "call_a".to_string(),
                name: "mcp__node_repl__js".to_string(),
                arguments: r#"{"code":"1+1"}"#.to_string(),
            }],
        );

        assert_eq!(body["output"][0]["type"], "function_call");
        assert_eq!(body["output"][0]["name"], "mcp__node_repl__js");
        assert_eq!(body["output"][0]["namespace"], "node_repl");
    }

    #[test]
    fn emits_reasoning_as_separate_output_item() {
        let item = reasoning_output_item("thinking only");

        assert_eq!(item["type"], "reasoning");
        assert_eq!(item["summary"][0]["text"], "thinking only");
    }

    #[test]
    fn echoes_response_tools() {
        let request = parse_request(json!({
            "model": "m",
            "input": "hi",
            "tools": [{
                "type": "function",
                "name": "search",
                "description": "Search docs",
                "parameters": {"type":"object"}
            }]
        }))
        .unwrap();
        let body = response(&request, "ok", &[]);

        assert_eq!(body["tools"][0]["type"], "function");
        assert_eq!(body["tools"][0]["name"], "search");
    }

    #[test]
    fn parses_tool_choice_and_parallel_flag() {
        let request = parse_request(json!({
            "model": "m",
            "input": "hi",
            "parallel_tool_calls": true,
            "tool_choice": {"type":"function","name":"search"}
        }))
        .unwrap();

        assert!(request.parallel_tool_calls);
        assert_eq!(request.tool_choice.required_name(), Some("search"));
    }

    #[test]
    fn parses_json_function_call_arguments_and_outputs() {
        let request = parse_request(json!({
            "model": "m",
            "input": [
                {"type":"function_call","call_id":"call_a","name":"search","arguments":{"q":"rs"}},
                {"type":"function_call_output","call_id":"call_a","output":{"ok":true}}
            ]
        }))
        .unwrap();

        assert!(request.messages[0].content_text().contains(r#""q":"rs""#));
        assert!(request.messages[1].content_text().contains(r#""ok":true"#));
    }

    #[test]
    fn parses_codex_structured_function_call_output_as_readable_text() {
        let request = parse_request(json!({
            "model": "m",
            "input": [
                {
                    "type":"function_call_output",
                    "call_id":"call_a",
                    "output":[
                        {"type":"input_text","text":"line one"},
                        {"type":"input_image","image_url":"data:image/png;base64,AAA"},
                        {"type":"encrypted_content","encrypted_content":"opaque"}
                    ]
                }
            ]
        }))
        .unwrap();
        let text = request.messages[0].content_text();

        assert!(text.contains("line one"));
        assert!(text.contains("[image: data:image/png;base64,AAA]"));
        assert!(text.contains("[encrypted_content]"));
    }
}
