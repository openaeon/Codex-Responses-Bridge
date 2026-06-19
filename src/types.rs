use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AdapterError;
use crate::wire::{chat, messages, responses, WireMode};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnifiedRequest {
    pub model: String,
    pub max_tokens: u32,
    pub system: Option<String>,
    pub messages: Vec<UnifiedMessage>,
    pub tools: Vec<ToolDefinition>,
    pub stream: bool,
    pub background: bool,
    pub previous_response_id: Option<String>,
}

impl UnifiedRequest {
    pub fn from_wire_payload(mode: WireMode, payload: Value) -> Result<Self, AdapterError> {
        match mode {
            WireMode::ChatCompletions => chat::parse_request(payload),
            WireMode::Messages => messages::parse_request(payload),
            WireMode::Responses => responses::parse_request(payload),
        }
    }

    pub fn render_prompt_with_tool_protocol(&self, protocol: &str) -> String {
        let mut sections = Vec::new();
        if let Some(system) = self
            .system
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            sections.push(format!("<system>\n{system}\n</system>"));
        }
        if !self.tools.is_empty() {
            sections.push(format!("<tool_protocol>\n{protocol}\n</tool_protocol>"));
        }
        for message in &self.messages {
            sections.push(format!("{}: {}", message.role, message.content_text()));
        }
        sections.join("\n\n")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnifiedMessage {
    pub role: String,
    pub content: Vec<UnifiedContent>,
}

impl UnifiedMessage {
    pub fn text(role: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: vec![UnifiedContent::Text { text: text.into() }],
        }
    }

    pub fn content_text(&self) -> String {
        self.content
            .iter()
            .map(UnifiedContent::as_prompt_text)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UnifiedContent {
    Text {
        text: String,
    },
    ImageUrl {
        url: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

impl UnifiedContent {
    fn as_prompt_text(&self) -> String {
        match self {
            Self::Text { text } => text.clone(),
            Self::ImageUrl { url } => format!("[Image: {url}]"),
            Self::ToolUse { id, name, input } => {
                format!("<previous_tool_call id=\"{id}\" name=\"{name}\">{input}</previous_tool_call>")
            }
            Self::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => format!(
                "<tool_result id=\"{tool_use_id}\" is_error=\"{is_error}\">\n{content}\n</tool_result>"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}
