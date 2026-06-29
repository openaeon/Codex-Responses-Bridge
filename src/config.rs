use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

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
    pub fn load_or_init(mut self) -> Result<Self, std::io::Error> {
        let mut local = LocalConfig::load_or_default()?;
        let changed = local.ensure_defaults();
        if changed {
            local.save()?;
        }

        self.adapter_api_key = choose_string(
            self.adapter_api_key,
            "ADAPTER_API_KEY",
            local.adapter_api_key.clone(),
        );
        if std::env::var_os("ADAPTER_UPSTREAM_MODEL").is_none()
            && self.upstream_model == "local-model"
        {
            if let Some(local_model) = local
                .upstream_model
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                self.upstream_model = local_model.to_string();
            }
        }
        self.model_aliases = choose_string(
            self.model_aliases,
            "ADAPTER_MODEL_ALIASES",
            local.model_aliases.clone().unwrap_or_default(),
        );
        if local.provider.as_deref() == Some("deepseek-web")
            && std::env::var_os("ADAPTER_UPSTREAM_MODEL").is_none()
            && self.upstream_model == "local-model"
        {
            self.upstream_model = "deepseek-web/reasoner".to_string();
        }
        Ok(self)
    }

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalConfig {
    pub adapter_api_key: String,
    pub provider: Option<String>,
    pub upstream_model: Option<String>,
    pub model_aliases: Option<String>,
}

impl Default for LocalConfig {
    fn default() -> Self {
        Self {
            adapter_api_key: String::new(),
            provider: Some("openai-compatible".to_string()),
            upstream_model: None,
            model_aliases: None,
        }
    }
}

impl LocalConfig {
    pub fn load_or_default() -> Result<Self, std::io::Error> {
        let path = local_config_path()?;
        match std::fs::read_to_string(&path) {
            Ok(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error),
        }
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = local_config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        std::fs::write(path, raw)
    }

    pub fn ensure_defaults(&mut self) -> bool {
        let mut changed = false;
        if self.adapter_api_key.trim().is_empty() {
            self.adapter_api_key = format!("adp_{}", Uuid::new_v4().simple());
            changed = true;
        }
        if self
            .provider
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            self.provider = Some("openai-compatible".to_string());
            changed = true;
        }
        changed
    }

    pub fn setup_json(&self, bind: &str) -> Value {
        json!({
            "config_file": local_config_path().ok(),
            "adapter_api_key": self.adapter_api_key,
            "provider": self.provider.as_deref().unwrap_or("openai-compatible"),
            "upstream_model": self.upstream_model.as_deref().unwrap_or("local-model"),
            "model_aliases": self.model_aliases.as_deref().unwrap_or(""),
            "adapter_base_url": format!("http://{bind}"),
            "openai_base_url": format!("http://{bind}/v1"),
        })
    }
}

pub fn local_config_path() -> Result<PathBuf, std::io::Error> {
    if let Some(path) = std::env::var_os("ADAPTER_CONFIG_FILE") {
        return Ok(PathBuf::from(path));
    }
    Ok(adapter_data_dir()?.join("config.json"))
}

pub fn response_store_path() -> Result<PathBuf, std::io::Error> {
    if let Some(path) = std::env::var_os("ADAPTER_RESPONSE_STORE_FILE") {
        return Ok(PathBuf::from(path));
    }
    Ok(adapter_data_dir()?.join("responses_store.json"))
}

pub fn adapter_data_dir() -> Result<PathBuf, std::io::Error> {
    if cfg!(windows) {
        if let Some(path) = std::env::var_os("APPDATA") {
            return Ok(PathBuf::from(path).join("model-toolcall-adapter"));
        }
    }
    Ok(user_home_dir()?.join(".model-toolcall-adapter"))
}

pub fn user_home_dir() -> Result<PathBuf, std::io::Error> {
    if let Some(home) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(home));
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        return Ok(PathBuf::from(profile));
    }
    match (std::env::var_os("HOMEDRIVE"), std::env::var_os("HOMEPATH")) {
        (Some(drive), Some(path)) => {
            let mut home = PathBuf::from(drive);
            home.push(path);
            Ok(home)
        }
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "cannot determine user home directory from HOME or USERPROFILE",
        )),
    }
}

pub fn update_local_config(
    f: impl FnOnce(&mut LocalConfig),
) -> Result<LocalConfig, std::io::Error> {
    let mut config = LocalConfig::load_or_default()?;
    config.ensure_defaults();
    f(&mut config);
    config.save()?;
    Ok(config)
}

fn choose_string(value: String, env_name: &str, local_value: String) -> String {
    if std::env::var_os(env_name).is_none() && value.trim().is_empty() {
        local_value
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::{local_config_path, user_home_dir, AppConfig, LocalConfig};
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

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

    #[test]
    fn local_config_generates_adapter_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        let path = std::env::temp_dir().join(format!(
            "model-toolcall-adapter-test-{}.json",
            uuid::Uuid::new_v4()
        ));
        std::env::set_var("ADAPTER_CONFIG_FILE", &path);

        let mut config = LocalConfig::load_or_default().unwrap();
        assert!(config.adapter_api_key.is_empty());
        assert!(config.ensure_defaults());
        config.save().unwrap();

        let loaded = LocalConfig::load_or_default().unwrap();
        assert!(loaded.adapter_api_key.starts_with("adp_"));
        assert_eq!(local_config_path().unwrap(), path);

        let _ = std::fs::remove_file(&path);
        std::env::remove_var("ADAPTER_CONFIG_FILE");
    }

    #[test]
    fn user_home_dir_falls_back_to_userprofile() {
        let _guard = ENV_LOCK.lock().unwrap();
        let old_home = std::env::var_os("HOME");
        let old_userprofile = std::env::var_os("USERPROFILE");
        std::env::remove_var("HOME");
        std::env::set_var("USERPROFILE", r"C:\Users\adapter");

        assert_eq!(
            user_home_dir().unwrap(),
            std::path::PathBuf::from(r"C:\Users\adapter")
        );

        if let Some(value) = old_home {
            std::env::set_var("HOME", value);
        }
        if let Some(value) = old_userprofile {
            std::env::set_var("USERPROFILE", value);
        } else {
            std::env::remove_var("USERPROFILE");
        }
    }
}
