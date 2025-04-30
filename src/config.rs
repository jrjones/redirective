// (C) Copyright 2025, Joseph R. Jones - https://jrj.org - Licensed under MIT License
//! config module: loads links from YAML and service settings.

use crate::errors::Error;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

/// Service configuration parameters.
pub struct ServiceConfig {
    /// The address (host:port) to bind the HTTP server to.
    pub address: String,
    /// Reload interval in seconds for Git sync.
    pub reload_interval_secs: u64,
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
        struct RawServiceConfig {
            address: Option<String>,
            reload_interval_secs: Option<u64>,
        }
        // Default settings
        let mut service = ServiceConfig {
            address: "0.0.0.0:8080".to_string(),
            reload_interval_secs: 60,
        };
        if let Ok(toml_str) = fs::read_to_string("redirective.toml") {
            let raw: RawServiceConfig = toml::from_str(&toml_str)?;
            if let Some(addr) = raw.address {
                service.address = addr;
            }
            if let Some(interval) = raw.reload_interval_secs {
                service.reload_interval_secs = interval;
            }
        }

        Ok(Config { links, service })
    }
}
