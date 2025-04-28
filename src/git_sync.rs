//! git_sync module: periodically pulls from Git and reloads config.

// Git sync pulls to update RouterCache periodically

use crate::cache::RouterCache;
/// Start the Git sync background task.
///
/// `links_path` is the path to `links.yaml`,
/// `reload_interval_secs` is how often to sync in seconds.
use crate::config::Config;
use crate::metrics::Metrics;
use std::process::Command;
use std::time::Duration;
use tokio::task;
use tokio::time;
use tracing::error;

/// Start the Git sync background task.
///
/// Periodically runs `git pull --ff-only` in the repo of `links_path`, then reloads links.yaml
/// and swaps the cache if parsing succeeds. Updates reload_success and reload_fail metrics.
pub fn start_git_sync(
    links_path: &str,
    cache: RouterCache,
    reload_interval_secs: u64,
    metrics: Metrics,
) {
    // Clone arguments into the async task
    let links_path = links_path.to_string();
    let cache = cache.clone();
    let metrics = metrics.clone();
    task::spawn(async move {
        loop {
            time::sleep(Duration::from_secs(reload_interval_secs)).await;
            // Perform git pull in the directory of links_path
            let repo_dir = std::path::Path::new(&links_path)
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."));
            match Command::new("git")
                .current_dir(repo_dir)
                .args(["pull", "--ff-only"])
                .status()
            {
                Ok(status) if status.success() => {
                    // Reload configuration
                    match Config::load(&links_path) {
                        Ok(cfg) => {
                            cache.swap(cfg.links);
                            metrics.reload_success.inc();
                        }
                        Err(err) => {
                            metrics.reload_fail.inc();
                            error!("config reload failed: {}", err);
                        }
                    }
                }
                Ok(status) => {
                    metrics.reload_fail.inc();
                    error!("git pull failed with status: {}", status);
                }
                Err(err) => {
                    metrics.reload_fail.inc();
                    error!("git pull error: {}", err);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::RouterCache;
    use crate::metrics::init_metrics;
    use std::{collections::HashMap, fs};
    // For temporary directories
    use std::env;
    use tokio::time::{Duration, sleep};

    #[tokio::test]
    async fn test_git_sync_fail_on_non_git_dir() {
        // Skip if git is not available
        if Command::new("git").arg("--version").output().is_err() {
            eprintln!("git not found; skipping test_git_sync_fail_on_non_git_dir");
            return;
        }
        // Setup a temp directory without a Git repo
        let mut tmp = env::temp_dir();
        tmp.push("redirective_test_git_sync");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir(&tmp).unwrap();
        let links_path = tmp.join("links.yaml");
        fs::write(&links_path, "foo: http://example.com/foo").unwrap();
        // Initialize cache and metrics
        let cache = RouterCache::new(HashMap::new());
        let metrics = init_metrics();
        // Start git-sync with short interval
        start_git_sync(links_path.to_str().unwrap(), cache, 1, metrics.clone());
        // Wait for at least one sync attempt
        sleep(Duration::from_secs(2)).await;
        // reload_fail should have incremented
        assert!(
            metrics.reload_fail.get() >= 1,
            "Expected reload_fail >= 1, got {}",
            metrics.reload_fail.get()
        );
    }

    #[tokio::test]
    async fn test_git_sync_success_updates_cache() {
        // Skip if git is not available
        if Command::new("git").arg("--version").output().is_err() {
            eprintln!("git not found; skipping test_git_sync_success_updates_cache");
            return;
        }
        // Setup temp directories
        let tmp = env::temp_dir().join("redirective_git_integration");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let origin = tmp.join("origin.git");
        // Initialize bare origin
        Command::new("git")
            .args(["init", "--bare", origin.to_str().unwrap()])
            .status()
            .expect("failed to init bare origin");
        // Determine default branch of bare repo (e.g., main or master)
        let head_file = origin.join("HEAD");
        let head_ref = fs::read_to_string(&head_file).unwrap();
        let default_branch = head_ref.trim().split('/').next_back().unwrap().to_string();
        // Initialize init_dir non-bare and push initial commit
        let init_dir = tmp.join("init");
        Command::new("git")
            .args(["init", init_dir.to_str().unwrap()])
            .status()
            .expect("failed to init init_dir");
        // Configure user identity for CI commits
        Command::new("git")
            .current_dir(&init_dir)
            .args(["config", "user.email", "ci@example.com"])
            .status()
            .unwrap();
        Command::new("git")
            .current_dir(&init_dir)
            .args(["config", "user.name", "ci"])
            .status()
            .unwrap();
        // write initial files
        let links_init = init_dir.join("links.yaml");
        fs::write(&links_init, "foo: http://initial").unwrap();
        let toml_init = init_dir.join("redirective.toml");
        fs::write(&toml_init, "reload_interval_secs = 1").unwrap();
        // git add, commit, and push
        Command::new("git")
            .current_dir(&init_dir)
            .args(["add", "links.yaml", "redirective.toml"])
            .status()
            .unwrap();
        Command::new("git")
            .current_dir(&init_dir)
            .args(["commit", "-m", "init"])
            .status()
            .unwrap();
        Command::new("git")
            .current_dir(&init_dir)
            .args(["remote", "add", "origin", origin.to_str().unwrap()])
            .status()
            .unwrap();
        // Push initial commit to remote master
        // Push initial commit to remote default branch
        Command::new("git")
            .current_dir(&init_dir)
            .args(["push", "origin", &format!("HEAD:{}", default_branch)])
            .status()
            .unwrap();
        // Clone to repo_dir
        let repo_dir = tmp.join("repo");
        Command::new("git")
            .args([
                "clone",
                origin.to_str().unwrap(),
                repo_dir.to_str().unwrap(),
            ])
            .status()
            .unwrap();
        // Prepare cache and metrics; switch CWD to repo_dir so redirective.toml is loaded from there
        let links_path = repo_dir.join("links.yaml");
        env::set_current_dir(&repo_dir).unwrap();
        let cfg = Config::load(links_path.to_str().unwrap()).unwrap();
        let cache = RouterCache::new(cfg.links.clone());
        let metrics = init_metrics();
        // Start git sync
        start_git_sync(
            links_path.to_str().unwrap(),
            cache.clone(),
            cfg.service.reload_interval_secs,
            metrics.clone(),
        );
        // Initial lookup
        assert_eq!(cache.lookup("foo"), Some("http://initial".to_string()));
        // Update via init_dir: modify links and push
        fs::write(&links_init, "bar: http://updated").unwrap();
        Command::new("git")
            .current_dir(&init_dir)
            .args(["add", "links.yaml"])
            .status()
            .unwrap();
        Command::new("git")
            .current_dir(&init_dir)
            .args(["commit", "-m", "update"])
            .status()
            .unwrap();
        // Push update commit to remote master
        // Push update commit to remote default branch
        Command::new("git")
            .current_dir(&init_dir)
            .args(["push", "origin", &format!("HEAD:{}", default_branch)])
            .status()
            .unwrap();
        // Wait for sync
        sleep(Duration::from_secs(2)).await;
        // Now cache should reflect new mapping
        assert_eq!(cache.lookup("bar"), Some("http://updated".to_string()));
        assert!(
            metrics.reload_success.get() >= 1,
            "Expected reload_success >=1"
        );
    }
}
