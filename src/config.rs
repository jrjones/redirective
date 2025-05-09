// (C) Copyright 2025, Joseph R. Jones - https://jrj.org - Licensed under MIT License
//! config module: loads links from YAML and service settings.

use crate::errors::Error;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

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
}

fn default_address() -> String {
    "0.0.0.0:8080".to_string()
}

fn default_webhook_path() -> String {
    "/git-webhook".to_string()
}

fn default_rate_limit_minute() -> u32 {
    1
}

fn default_rate_limit_day() -> u32 {
    100
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
}

#[derive(Deserialize)]
struct RawServiceConfig {
    address: Option<String>,
    webhook: Option<RawWebhookConfig>,
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
            }
        }

        Ok(Config { links, service })
    }
}
