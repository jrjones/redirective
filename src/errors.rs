// (C) Copyright 2025, Joseph R. Jones - https://jrj.org - Licensed under MIT License
//! errors module: central error type for redirective service.

/// A generic error type for the application.
pub type Error = Box<dyn std::error::Error + Send + Sync>;
