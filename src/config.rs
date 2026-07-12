// (C) Copyright 2025, Joseph R. Jones - https://jrj.org - Licensed under MIT License
//! config module: loads links from YAML and service settings.

use crate::errors::Error;
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::str::FromStr;

/// Links YAML structure with support for comments.
#[derive(Deserialize, Debug)]
#[serde(transparent)]
pub struct Links {
    /// Mapping of codes to target URLs.
    pub links: HashMap<String, String>,
}

/// Service configuration parameters.
#[derive(Clone, Deserialize)]
pub struct ServiceConfig {
    /// The address (host:port) to bind the HTTP server to.
    #[serde(default = "default_address")]
    pub address: String,
    /// (Deprecated) Reload interval no longer used; webhook triggers reload.
    // pub reload_interval_secs: u64,
    /// HTTP path for the reload webhook endpoint.
    #[serde(default = "default_webhook_path")]
    pub webhook_path: String,
    /// Max reload webhook requests per minute per IP.
    #[serde(default = "default_rate_limit_minute")]
    pub rate_limit_per_minute: u32,
    /// Max reload webhook requests per day per IP.
    #[serde(default = "default_rate_limit_day")]
    pub rate_limit_per_day: u32,
    /// Optional peer webhook URL to relay reload webhooks to (e.g. the
    /// standby node's `/git-webhook` endpoint). Absent = relay disabled.
    #[serde(default)]
    pub peer_url: Option<String>,
    /// Interval, in seconds, between background `git pull` + reload polls.
    /// `None` (or `0`) disables polling.
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: Option<u64>,
}

fn default_address() -> String {
    "0.0.0.0:8080".to_string()
}

fn default_webhook_path() -> String {
    "/git-webhook".to_string()
}

fn default_rate_limit_minute() -> u32 {
    30
}

fn default_rate_limit_day() -> u32 {
    100
}

fn default_poll_interval_secs() -> Option<u64> {
    Some(60)
}

/// Overall application configuration.
pub struct Config {
    /// Mapping of codes to target URLs.
    pub links: HashMap<String, String>,
    /// Service settings.
    pub service: ServiceConfig,
}

#[derive(Deserialize)]
struct RawWebhookConfig {
    path: Option<String>,
    rate_limit_per_minute: Option<u32>,
    rate_limit_per_day: Option<u32>,
    peer_url: Option<String>,
}

#[derive(Deserialize)]
struct RawPollConfig {
    interval_secs: Option<u64>,
}

#[derive(Deserialize)]
struct RawServiceConfig {
    address: Option<String>,
    webhook: Option<RawWebhookConfig>,
    poll: Option<RawPollConfig>,
}

impl Config {
    /// Load configuration from `links.yaml` and optional service settings.
    pub fn load(links_path: &str) -> Result<Self, Error> {
        // Read links file and parse mappings
        let content = fs::read_to_string(links_path)?;

        // Parse YAML using serde_yaml
        let links_data: Links = serde_yaml::from_str(&content)
            .map_err(|e| Error::Config(format!("Failed to parse YAML: {}", e)))?;

        let links = links_data.links;

        // Validate URLs are not empty
        for (key, url) in &links {
            if url.trim().is_empty() {
                return Err(Error::Config(format!("Empty URL for key '{}'", key)));
            }
        }

        // Default settings
        let mut service = ServiceConfig {
            address: default_address(),
            webhook_path: default_webhook_path(),
            rate_limit_per_minute: default_rate_limit_minute(),
            rate_limit_per_day: default_rate_limit_day(),
            peer_url: None,
            poll_interval_secs: default_poll_interval_secs(),
        };

        // Read service settings from redirective.toml, if available
        if let Ok(toml_str) = fs::read_to_string("redirective.toml") {
            let raw: RawServiceConfig = toml::from_str(&toml_str)?;
            if let Some(addr) = raw.address {
                service.address = addr;
            }
            if let Some(webhook_raw) = raw.webhook {
                if let Some(path) = webhook_raw.path {
                    service.webhook_path = path;
                }
                if let Some(min) = webhook_raw.rate_limit_per_minute {
                    service.rate_limit_per_minute = min;
                }
                if let Some(day) = webhook_raw.rate_limit_per_day {
                    service.rate_limit_per_day = day;
                }
                if let Some(peer) = webhook_raw.peer_url {
                    service.peer_url = Some(peer);
                }
            }
            if let Some(poll_raw) = raw.poll
                && let Some(secs) = poll_raw.interval_secs
            {
                service.poll_interval_secs = Some(secs);
            }
        }

        apply_env_overrides(&mut service);

        Ok(Config { links, service })
    }
}

/// Parse an env var of type `T`, logging and ignoring it if present but
/// malformed rather than panicking.
fn env_override<T: FromStr>(key: &str) -> Option<T> {
    let raw = env::var(key).ok()?;
    match raw.parse::<T>() {
        Ok(v) => Some(v),
        Err(_) => {
            tracing::warn!(key, value = %raw, "ignoring malformed env override");
            None
        }
    }
}

/// Apply `REDIRECTIVE_*` env var overrides on top of TOML/defaults. Env wins
/// over TOML, which wins over the built-in default.
fn apply_env_overrides(service: &mut ServiceConfig) {
    if let Ok(raw) = env::var("REDIRECTIVE_POLL_INTERVAL_SECS") {
        match raw.parse::<u64>() {
            Ok(0) => service.poll_interval_secs = None,
            Ok(secs) => service.poll_interval_secs = Some(secs),
            Err(_) => tracing::warn!(
                value = %raw,
                "ignoring malformed REDIRECTIVE_POLL_INTERVAL_SECS"
            ),
        }
    }
    if let Some(min) = env_override::<u32>("REDIRECTIVE_RATE_LIMIT_PER_MINUTE") {
        service.rate_limit_per_minute = min;
    }
    if let Some(day) = env_override::<u32>("REDIRECTIVE_RATE_LIMIT_PER_DAY") {
        service.rate_limit_per_day = day;
    }
    if let Ok(peer) = env::var("REDIRECTIVE_PEER_URL") {
        let trimmed = peer.trim();
        service.peer_url = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    /// Env vars are process-global; serialize the tests that touch them so
    /// they don't clobber each other when run concurrently.
    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvGuard {
        key: &'static str,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            unsafe { env::set_var(key, value) };
            EnvGuard { key }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            unsafe { env::remove_var(self.key) };
        }
    }

    fn baseline_service() -> ServiceConfig {
        ServiceConfig {
            address: default_address(),
            webhook_path: default_webhook_path(),
            rate_limit_per_minute: default_rate_limit_minute(),
            rate_limit_per_day: default_rate_limit_day(),
            peer_url: None,
            poll_interval_secs: default_poll_interval_secs(),
        }
    }

    #[test]
    fn test_default_rate_limit_per_minute_is_thirty() {
        assert_eq!(default_rate_limit_minute(), 30);
    }

    #[test]
    fn test_env_override_beats_toml_and_default() {
        let _lock = env_lock().lock().unwrap();
        let _guard = EnvGuard::set("REDIRECTIVE_RATE_LIMIT_PER_MINUTE", "5");
        let mut service = baseline_service();
        // Simulate a value already applied from redirective.toml.
        service.rate_limit_per_minute = 99;
        apply_env_overrides(&mut service);
        assert_eq!(service.rate_limit_per_minute, 5);
    }

    #[test]
    fn test_no_env_keeps_existing_value() {
        let _lock = env_lock().lock().unwrap();
        unsafe { env::remove_var("REDIRECTIVE_RATE_LIMIT_PER_MINUTE") };
        let mut service = baseline_service();
        service.rate_limit_per_minute = 7;
        apply_env_overrides(&mut service);
        assert_eq!(service.rate_limit_per_minute, 7);
    }

    #[test]
    fn test_malformed_env_falls_back_to_current_value() {
        let _lock = env_lock().lock().unwrap();
        let _guard = EnvGuard::set("REDIRECTIVE_RATE_LIMIT_PER_MINUTE", "not-a-number");
        let mut service = baseline_service();
        service.rate_limit_per_minute = 42;
        apply_env_overrides(&mut service);
        assert_eq!(service.rate_limit_per_minute, 42);
    }

    #[test]
    fn test_poll_interval_env_zero_disables() {
        let _lock = env_lock().lock().unwrap();
        let _guard = EnvGuard::set("REDIRECTIVE_POLL_INTERVAL_SECS", "0");
        let mut service = baseline_service();
        apply_env_overrides(&mut service);
        assert_eq!(service.poll_interval_secs, None);
    }

    #[test]
    fn test_poll_interval_env_sets_value() {
        let _lock = env_lock().lock().unwrap();
        let _guard = EnvGuard::set("REDIRECTIVE_POLL_INTERVAL_SECS", "45");
        let mut service = baseline_service();
        apply_env_overrides(&mut service);
        assert_eq!(service.poll_interval_secs, Some(45));
    }

    #[test]
    fn test_poll_interval_malformed_env_falls_back() {
        let _lock = env_lock().lock().unwrap();
        let _guard = EnvGuard::set("REDIRECTIVE_POLL_INTERVAL_SECS", "soon");
        let mut service = baseline_service();
        service.poll_interval_secs = Some(120);
        apply_env_overrides(&mut service);
        assert_eq!(service.poll_interval_secs, Some(120));
    }

    #[test]
    fn test_peer_url_env_empty_disables() {
        let _lock = env_lock().lock().unwrap();
        let _guard = EnvGuard::set("REDIRECTIVE_PEER_URL", "   ");
        let mut service = baseline_service();
        service.peer_url = Some("https://old/git-webhook".to_string());
        apply_env_overrides(&mut service);
        assert_eq!(service.peer_url, None);
    }

    #[test]
    fn test_peer_url_env_sets_value() {
        let _lock = env_lock().lock().unwrap();
        let _guard = EnvGuard::set("REDIRECTIVE_PEER_URL", "https://peer/git-webhook");
        let mut service = baseline_service();
        apply_env_overrides(&mut service);
        assert_eq!(
            service.peer_url,
            Some("https://peer/git-webhook".to_string())
        );
    }
}
