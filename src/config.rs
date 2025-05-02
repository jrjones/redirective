// (C) Copyright 2025, Joseph R. Jones - https://jrj.org - Licensed under MIT License
//! config module: loads links from YAML and service settings.

use crate::errors::Error;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

/// Service configuration parameters.
#[derive(Clone)]
pub struct ServiceConfig {
    /// The address (host:port) to bind the HTTP server to.
    pub address: String,
    /// (Deprecated) Reload interval no longer used; webhook triggers reload.
    // pub reload_interval_secs: u64,
    /// HTTP path for the reload webhook endpoint.
    pub webhook_path: String,
    /// Max reload webhook requests per minute per IP.
    pub rate_limit_per_minute: u32,
    /// Max reload webhook requests per day per IP.
    pub rate_limit_per_day: u32,
}

/// Overall application configuration.
pub struct Config {
    /// Mapping of codes to target URLs.
    pub links: HashMap<String, String>,
    /// Service settings.
    pub service: ServiceConfig,
}

impl Config {
    /// Load configuration from `links.yaml` and optional service settings.
    pub fn load(links_path: &str) -> Result<Self, Error> {
        // Read links file and parse mappings
        let content = fs::read_to_string(links_path)?;
        let mut links = HashMap::new();
        for (idx, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Expect format: key: value [| comment]
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() != 2 {
                return Err(format!("Invalid line in links.yaml at {}: {}", idx + 1, line).into());
            }
            let key = parts[0].trim().to_string();
            let rest = parts[1].trim();
            // Strip inline comments after '|'
            // Take part before inline comment and trim
            let mut url = rest.split('|').next().unwrap().trim().to_string();
            // Strip surrounding single or double quotes, if present
            url = url.trim_matches(|c| c == '"' || c == '\'').to_string();
            if url.is_empty() {
                return Err(format!(
                    "Empty URL for key '{}' in links.yaml at line {}",
                    key,
                    idx + 1
                )
                .into());
            }
            links.insert(key, url);
        }

        // Read service settings from redirective.toml, if available
        #[derive(Deserialize)]
        struct RawWebhookConfig {
            path: Option<String>,
            rate_limit_per_minute: Option<u32>,
            rate_limit_per_day: Option<u32>,
        }

        #[derive(Deserialize)]
        struct RawServiceConfig {
            address: Option<String>,
            webhook: Option<RawWebhookConfig>,
        }
        // Default settings
        let mut service = ServiceConfig {
            address: "0.0.0.0:8080".to_string(),
            webhook_path: "/git-webhook".to_string(),
            rate_limit_per_minute: 1,
            rate_limit_per_day: 100,
        };
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
            }
        }

        Ok(Config { links, service })
    }
}
