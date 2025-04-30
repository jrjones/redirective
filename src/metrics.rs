// (C) Copyright 2025, Joseph R. Jones - https://jrj.org - Licensed under MIT License
//! metrics module: Prometheus metrics for redirective service.

use prometheus::{HistogramVec, IntCounter, IntCounterVec, Registry};
use std::sync::Arc;

/// Prometheus metrics handles.
#[derive(Clone)]
pub struct Metrics {
    /// Counter of redirects per code.
    pub redirect_total: IntCounterVec,
    /// Histogram of redirect latencies per code.
    pub redirect_latency: HistogramVec,
    /// Counter of successful config reloads.
    pub reload_success: IntCounter,
    /// Counter of failed config reloads.
    pub reload_fail: IntCounter,
    /// The registry holding all metrics.
    pub registry: Arc<Registry>,
}

/// Initialize and register Prometheus metrics.
pub fn init_metrics() -> Metrics {
    // Create a Prometheus registry
    let registry = Registry::new();
    // Counter of redirects per code label
    let redirect_total = IntCounterVec::new(
        prometheus::Opts::new("redirect_total", "Counter of redirects per code"),
        &["code"],
    )
    .expect("failed to create redirect_total metric");
    registry
        .register(Box::new(redirect_total.clone()))
        .expect("failed to register redirect_total");
    // Histogram of redirect latencies per code
    let redirect_latency = HistogramVec::new(
        prometheus::HistogramOpts::new(
            "redirect_latency_seconds",
            "Histogram of redirect latencies per code",
        ),
        &["code"],
    )
    .expect("failed to create redirect_latency metric");
    registry
        .register(Box::new(redirect_latency.clone()))
        .expect("failed to register redirect_latency");
    // Counter of successful reloads
    let reload_success = IntCounter::new("reload_success", "Counter of successful config reloads")
        .expect("failed to create reload_success metric");
    registry
        .register(Box::new(reload_success.clone()))
        .expect("failed to register reload_success");
    // Counter of failed reloads
    let reload_fail = IntCounter::new("reload_fail", "Counter of failed config reloads")
        .expect("failed to create reload_fail metric");
    registry
        .register(Box::new(reload_fail.clone()))
        .expect("failed to register reload_fail");
    Metrics {
        redirect_total,
        redirect_latency,
        reload_success,
        reload_fail,
        registry: Arc::new(registry),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_metrics_registration() {
        let metrics = init_metrics();
        // Use each metric at least once to ensure it's emitted
        let _ = metrics.redirect_total.with_label_values(&["x"]);
        metrics
            .redirect_latency
            .with_label_values(&["x"])
            .observe(0.0);
        metrics.reload_success.inc();
        metrics.reload_fail.inc();
        let families = metrics.registry.gather();
        let names: Vec<_> = families.iter().map(|f| f.name()).collect();
        // Ensure counters are registered
        assert!(names.contains(&"redirect_total"));
        assert!(names.contains(&"reload_success"));
        assert!(names.contains(&"reload_fail"));
        // Histogram produces bucket, sum, and count families
        assert!(
            names
                .iter()
                .any(|n| n.starts_with("redirect_latency_seconds"))
        );
    }

    #[test]
    fn test_redirect_total_label() {
        let metrics = init_metrics();
        // Create a counter entry for code 'x'
        let _ = metrics.redirect_total.with_label_values(&["x"]);
        // Gather and find metric family
        let families = metrics.registry.gather();
        let family = families
            .into_iter()
            .find(|f| f.name() == "redirect_total")
            .expect("redirect_total not found");
        // Ensure there is at least one metric with the label 'code'='x'
        let metrics_vec = family.get_metric();
        assert!(metrics_vec.iter().any(|m| {
            m.get_label()
                .iter()
                .any(|l| l.name() == "code" && l.value() == "x")
        }));
    }
}
