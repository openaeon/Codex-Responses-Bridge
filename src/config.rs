use clap::Parser;
use std::collections::HashMap;

#[derive(Debug, Clone, Parser)]
#[command(name = "model-toolcall-adapter-rs")]
#[command(about = "OpenAI-style tool-call adapter for models without native tool calling.")]
pub struct AppConfig {
    #[arg(long, env = "ADAPTER_BIND", default_value = "127.0.0.1:8787")]
    pub bind: String,

    #[arg(
        long,
        env = "ADAPTER_UPSTREAM_BASE_URL",
        default_value = "http://127.0.0.1:11434/v1"
    )]
    pub upstream_base_url: String,

    #[arg(long, env = "ADAPTER_UPSTREAM_API_KEY", default_value = "")]
    pub upstream_api_key: String,

    #[arg(long, env = "ADAPTER_UPSTREAM_MODEL", default_value = "local-model")]
    pub upstream_model: String,

    #[arg(long, env = "ADAPTER_MODEL_ALIASES", default_value = "")]
    pub model_aliases: String,

    #[arg(long, env = "ADAPTER_API_KEY", default_value = "")]
    pub adapter_api_key: String,

    #[arg(long, env = "ADAPTER_MAX_TOOL_DEFINITIONS", default_value_t = 64)]
    pub max_tool_definitions: usize,

    #[arg(long, env = "ADAPTER_REQUEST_TIMEOUT_SECS", default_value_t = 120)]
    pub request_timeout_secs: u64,
}

impl AppConfig {
    pub fn model_alias_map(&self) -> HashMap<String, String> {
        self.model_aliases
            .split(',')
            .filter_map(|entry| {
                let (alias, upstream) = entry.split_once('=')?;
                let alias = alias.trim();
                let upstream = upstream.trim();
                (!alias.is_empty() && !upstream.is_empty())
                    .then(|| (alias.to_string(), upstream.to_string()))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn parses_model_aliases() {
        let config = AppConfig {
            bind: "127.0.0.1:8787".to_string(),
            upstream_base_url: "http://127.0.0.1:11434/v1".to_string(),
            upstream_api_key: String::new(),
            upstream_model: "deepseek-web/reasoner".to_string(),
            model_aliases: "codex=deepseek-web/reasoner,fast=deepseek-web/chat".to_string(),
            adapter_api_key: String::new(),
            max_tool_definitions: 64,
            request_timeout_secs: 120,
        };

        let aliases = config.model_alias_map();

        assert_eq!(aliases.get("codex").unwrap(), "deepseek-web/reasoner");
        assert_eq!(aliases.get("fast").unwrap(), "deepseek-web/chat");
    }
}
