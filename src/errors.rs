//! errors module: central error type for redirective service.

/// A generic error type for the application.
pub type Error = Box<dyn std::error::Error + Send + Sync>;