use std::collections::HashMap;
use std::time::Duration;

use reqwest::Client;
use serde_json::{json, Value};

use crate::config::AppConfig;
use crate::error::AdapterError;
use crate::types::UnifiedRequest;

#[derive(Debug, Clone)]
pub struct UpstreamResponse {
    pub text: String,
    pub reasoning: Option<String>,
}

#[derive(Clone)]
pub struct OpenAiChatUpstream {
    client: Client,
    base_url: String,
    api_key: String,
}

#[derive(Debug, Clone, Default)]
pub struct UpstreamRequestOptions {
    pub provider: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub deepseek_session: Option<String>,
}

impl OpenAiChatUpstream {
    pub fn new(config: &AppConfig) -> Result<Self, AdapterError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_secs))
            .build()
            .map_err(|error| AdapterError::Upstream(error.to_string()))?;
        Ok(Self {
            client,
            base_url: config.upstream_base_url.trim_end_matches('/').to_string(),
            api_key: config.upstream_api_key.clone(),
        })
    }

    pub async fn complete(
        &self,
        request: &UnifiedRequest,
        prompt: &str,
        options: &UpstreamRequestOptions,
    ) -> Result<UpstreamResponse, AdapterError> {
        if request.model.starts_with("deepseek-web/")
            || options
                .provider
                .as_deref()
                .is_some_and(|provider| provider.eq_ignore_ascii_case("deepseek-web"))
        {
            return self.deepseek_web_complete(request, prompt, options).await;
        }

        let base_url = self.effective_base_url(options);
        let api_key = self.effective_api_key(options);
        let endpoint = chat_completions_endpoint(&base_url);
        let body = json!({
            "model": request.model,
            "stream": false,
            "max_tokens": request.max_tokens,
            "messages": [{ "role": "user", "content": prompt }]
        });
        let mut builder = self.client.post(endpoint).json(&body);
        if !api_key.trim().is_empty() {
            builder = builder.bearer_auth(api_key);
        }
        let response = builder
            .send()
            .await
            .map_err(|error| AdapterError::Upstream(error.to_string()))?;
        let status = response.status();
        let payload = response
            .json::<Value>()
            .await
            .map_err(|error| AdapterError::Upstream(error.to_string()))?;
        if !status.is_success() {
            return Err(AdapterError::Upstream(payload.to_string()));
        }
        let text = payload
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| {
                payload
                    .get("response")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .ok_or_else(|| AdapterError::Upstream(format!("missing assistant text: {payload}")))?;

        let reasoning = payload
            .pointer("/choices/0/message/reasoning_content")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| {
                payload
                    .pointer("/choices/0/message/reasoning")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            });

        Ok(UpstreamResponse { text, reasoning })
    }

    pub async fn list_models(
        &self,
        fallback_model: &str,
        model_aliases: &HashMap<String, String>,
        options: &UpstreamRequestOptions,
    ) -> Result<Value, AdapterError> {
        if fallback_model.starts_with("deepseek-web/")
            || options
                .provider
                .as_deref()
                .is_some_and(|provider| provider.eq_ignore_ascii_case("deepseek-web"))
        {
            let mut data = vec![
                json!({ "id": "deepseek-web/reasoner", "object": "model", "owned_by": "deepseek-web" }),
                json!({ "id": "deepseek-web/chat", "object": "model", "owned_by": "deepseek-web" }),
            ];
            data.extend(alias_model_items(model_aliases));
            return Ok(json!({ "object": "list", "data": data }));
        }

        let base_url = self.effective_base_url(options);
        let api_key = self.effective_api_key(options);
        let endpoint = models_endpoint(&base_url);
        let mut builder = self.client.get(endpoint);
        if !api_key.trim().is_empty() {
            builder = builder.bearer_auth(api_key);
        }
        let response = match builder.send().await {
            Ok(response) => response,
            Err(error) => {
                return Ok(fallback_models(
                    fallback_model,
                    model_aliases,
                    error.to_string(),
                ))
            }
        };
        let status = response.status();
        let payload = match response.json::<Value>().await {
            Ok(payload) => payload,
            Err(error) => {
                return Ok(fallback_models(
                    fallback_model,
                    model_aliases,
                    error.to_string(),
                ))
            }
        };
        if status.is_success() {
            return Ok(with_alias_models(payload, model_aliases));
        }
        Ok(fallback_models(
            fallback_model,
            model_aliases,
            payload.to_string(),
        ))
    }

    pub async fn fetch_market_overview(
        &self,
        base_url: &str,
        symbol: &str,
        market: &str,
        modules: &[String],
    ) -> Result<Value, AdapterError> {
        let endpoint = market_overview_endpoint(base_url);
        let modules_csv = modules.join(",");
        let mut last_error = None;
        let response = 'retry: {
            for attempt in 1..=3 {
                match self
                    .client
                    .get(&endpoint)
                    .query(&[
                        ("symbol", symbol),
                        ("market", market),
                        ("modules", modules_csv.as_str()),
                    ])
                    .send()
                    .await
                {
                    Ok(response) => break 'retry response,
                    Err(error) => {
                        last_error = Some(error.to_string());
                        if attempt < 3 {
                            tokio::time::sleep(Duration::from_millis(250 * attempt)).await;
                        }
                    }
                }
            }
            return Err(AdapterError::Upstream(format!(
                "market overview request failed after retries: endpoint={endpoint}, symbol={symbol}, market={market}, modules={modules_csv}, error={}",
                last_error.unwrap_or_else(|| "unknown request error".to_string())
            )));
        };
        let status = response.status();
        let payload = response.json::<Value>().await.map_err(|error| {
            AdapterError::Upstream(format!(
                "market overview response json: endpoint={endpoint}, error={error}"
            ))
        })?;
        if !status.is_success() {
            return Err(AdapterError::Upstream(format!(
                "market overview returned HTTP {status}: endpoint={endpoint}, body={payload}"
            )));
        }
        Ok(payload)
    }

    async fn deepseek_web_complete(
        &self,
        request: &UnifiedRequest,
        prompt: &str,
        options: &UpstreamRequestOptions,
    ) -> Result<UpstreamResponse, AdapterError> {
        use aeon_claw_api::{
            AnthropicClient, AuthSource, InputContentBlock, InputMessage, MessageRequest,
        };

        let session = raw_deepseek_session_from_options(options)?;
        let client = AnthropicClient::from_auth(AuthSource::ApiKey(session))
            .with_base_url("https://chat.deepseek.com".to_string())
            .with_provider_hint(Some("deepseek_web".to_string()));
        let response = client
            .send_message(&MessageRequest {
                model: request.model.clone(),
                max_tokens: request.max_tokens,
                messages: vec![InputMessage {
                    role: "user".to_string(),
                    content: vec![InputContentBlock::Text {
                        text: prompt.to_string(),
                    }],
                }],
                system: None,
                tools: None,
                tool_choice: None,
                stream: false,
            })
            .await
            .map_err(|error| {
                let text = error.to_string();
                if is_deepseek_auth_error(&text) {
                    deepseek_session_expired_error()
                } else {
                    AdapterError::Upstream(format!("deepseek web provider: {text}"))
                }
            })?;

        let mut output = String::new();
        let mut reasoning = String::new();
        for block in response.content {
            match block {
                aeon_claw_api::OutputContentBlock::Text { text } => {
                    output.push_str(&text);
                }
                aeon_claw_api::OutputContentBlock::Thinking { thinking, .. } => {
                    reasoning.push_str(&thinking);
                }
                _ => {}
            }
        }
        if output.trim().is_empty() && reasoning.trim().is_empty() {
            return Err(AdapterError::Upstream(
                "deepseek web provider returned empty text and reasoning".to_string(),
            ));
        }
        let text = clean_deepseek_output(&output);
        let reasoning = (!reasoning.trim().is_empty()).then(|| reasoning.trim().to_string());
        Ok(UpstreamResponse { text, reasoning })
    }

    fn effective_base_url(&self, options: &UpstreamRequestOptions) -> String {
        options
            .base_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&self.base_url)
            .trim_end_matches('/')
            .to_string()
    }

    fn effective_api_key<'a>(&'a self, options: &'a UpstreamRequestOptions) -> &'a str {
        options
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&self.api_key)
    }
}

fn raw_deepseek_session_from_options(
    options: &UpstreamRequestOptions,
) -> Result<String, AdapterError> {
    options
        .deepseek_session
        .as_deref()
        .or(options.api_key.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(read_default_deepseek_session)
        .ok_or_else(|| {
            AdapterError::Upstream(
                "DeepSeek Web session missing. Paste session JSON/Cookie or run FCACoreai DeepSeek login first.".to_string(),
            )
        })
}

fn read_default_deepseek_session() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    std::fs::read_to_string(format!("{home}/.FCACore/deepseek_session.json")).ok()
}

fn clean_deepseek_output(text: &str) -> String {
    text.trim_end_matches("FINISHED").trim_end().to_string()
}

fn is_deepseek_auth_error(text: &str) -> bool {
    text.contains("Authorization Failed")
        || text.contains("invalid token")
        || text.contains("\"code\":40003")
}

fn deepseek_session_expired_error() -> AdapterError {
    AdapterError::Upstream(
        "DeepSeek Web session invalid or expired. Re-login DeepSeek Web and refresh ~/.FCACore/deepseek_session.json, or paste a fresh session JSON/Cookie in the UI.".to_string(),
    )
}

fn chat_completions_endpoint(base_url: &str) -> String {
    let lower = base_url.to_ascii_lowercase();
    if lower.ends_with("/chat/completions") {
        base_url.to_string()
    } else if lower.ends_with("/v1") {
        format!("{base_url}/chat/completions")
    } else {
        format!("{base_url}/v1/chat/completions")
    }
}

fn models_endpoint(base_url: &str) -> String {
    let lower = base_url.to_ascii_lowercase();
    if lower.ends_with("/models") {
        base_url.to_string()
    } else if lower.ends_with("/v1") {
        format!("{base_url}/models")
    } else {
        format!("{base_url}/v1/models")
    }
}

fn market_overview_endpoint(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/overview") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/overview")
    }
}

fn alias_model_items(model_aliases: &HashMap<String, String>) -> Vec<Value> {
    model_aliases
        .keys()
        .map(|alias| {
            json!({
                "id": alias,
                "object": "model",
                "owned_by": "adapter-alias"
            })
        })
        .collect()
}

fn with_alias_models(mut payload: Value, model_aliases: &HashMap<String, String>) -> Value {
    if let Some(data) = payload.get_mut("data").and_then(Value::as_array_mut) {
        data.extend(alias_model_items(model_aliases));
    }
    payload
}

fn fallback_models(
    fallback_model: &str,
    model_aliases: &HashMap<String, String>,
    warning: impl Into<String>,
) -> Value {
    let mut data = vec![json!({
        "id": fallback_model,
        "object": "model",
        "owned_by": "adapter-fallback"
    })];
    data.extend(alias_model_items(model_aliases));
    json!({
        "object": "list",
        "data": data,
        "warning": warning.into()
    })
}
