// (C) Copyright 2025, Joseph R. Jones - https://jrj.org - Licensed under MIT License
//! errors module: central error type for redirective service.

use hyper::Error as HyperError;
use std::net::AddrParseError;
use thiserror::Error;

/// Typed errors for the application
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parsing error: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    #[error("TOML parsing error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("Address parsing error: {0}")]
    AddrParse(#[from] AddrParseError),

    #[error("HTTP server error: {0}")]
    Http(#[from] HyperError),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Other error: {0}")]
    Other(String),
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Other(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::Other(s.to_string())
    }
}
