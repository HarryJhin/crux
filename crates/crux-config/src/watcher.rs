//! File-system watcher for hot-reloading configuration changes.
//!
//! Watches the config file's parent directory (to handle atomic saves where
//! editors write a temp file then rename it) and debounces events with a
//! 500ms window. Consumers receive [`ConfigEvent`] values over a
//! [`std::sync::mpsc::Receiver`].

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};

use crate::{ConfigError, CruxConfig};

/// Events emitted by the [`ConfigWatcher`].
#[derive(Debug)]
pub enum ConfigEvent {
    /// The config file was modified and successfully reloaded.
    Reloaded(Box<CruxConfig>),
    /// The config file was modified but failed to load.
    Error(ConfigError),
}

/// Watches a configuration file for changes and sends reload events.
///
/// The watcher monitors the parent directory of the config file so that
/// atomic save operations (write-to-temp + rename) are detected correctly.
/// Events are debounced with a 500ms window to avoid redundant reloads.
///
/// # Drop behavior
///
/// Dropping the `ConfigWatcher` stops the underlying file-system watcher.
/// The receiver channel will then yield `Err(RecvError)` on the next read.
pub struct ConfigWatcher {
    /// Kept alive to maintain the watch; dropped on `ConfigWatcher` drop.
    _watcher: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
}

impl ConfigWatcher {
    /// Start watching the given config file path.
    ///
    /// Returns the watcher handle and a receiver for [`ConfigEvent`]s.
    /// If the config file does not exist yet, the watcher still monitors
    /// the parent directory and will fire when the file is created.
    pub fn new(config_path: PathBuf) -> Result<(Self, mpsc::Receiver<ConfigEvent>), ConfigError> {
        let (event_tx, event_rx) = mpsc::channel();

        // Determine the directory to watch.
        let watch_dir = config_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        // Ensure the watch directory exists (config file itself may not).
        if !watch_dir.exists() {
            std::fs::create_dir_all(&watch_dir).map_err(ConfigError::IoError)?;
        }

        // Canonicalize watch_dir so event paths match our canonical config path.
        let watch_dir = watch_dir.canonicalize().unwrap_or(watch_dir);

        // Canonicalize the config path to resolve symlinks (e.g. /var → /private/var on macOS).
        // Fall back to the original path if canonicalization fails (file may not exist yet).
        let canonical_config = config_path.canonicalize().unwrap_or_else(|_| config_path.clone());
        let config_path_for_closure = canonical_config.clone();

        let mut debouncer = new_debouncer(
            Duration::from_millis(500),
            move |result: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
                match result {
                    Ok(events) => {
                        // Check if any event relates to our config file.
                        // Compare using canonicalized paths to handle symlinks.
                        let config_changed = events.iter().any(|e| {
                            e.kind == DebouncedEventKind::Any
                                && (e.path == config_path_for_closure
                                    || e.path.canonicalize().ok().as_ref() == Some(&config_path_for_closure))
                        });

                        if config_changed {
                            let event = match CruxConfig::load_from(&config_path_for_closure) {
                                Ok(config) => ConfigEvent::Reloaded(Box::new(config)),
                                Err(e) => ConfigEvent::Error(e),
                            };
                            // Ignore send error — receiver may have been dropped.
                            let _ = event_tx.send(event);
                        }
                    }
                    Err(e) => {
                        let _ = event_tx.send(ConfigEvent::Error(ConfigError::WatchError(
                            e.to_string(),
                        )));
                    }
                }
            },
        )
        .map_err(|e| ConfigError::WatchError(e.to_string()))?;

        debouncer
            .watcher()
            .watch(&watch_dir, notify::RecursiveMode::NonRecursive)
            .map_err(|e| ConfigError::WatchError(e.to_string()))?;

        log::info!(
            "Config watcher started for {} (canonical: {})",
            config_path.display(),
            canonical_config.display()
        );

        Ok((
            Self {
                _watcher: debouncer,
            },
            event_rx,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_config_watcher_creation_and_teardown() {
        let tmp_dir = std::env::temp_dir().join(format!("crux-watcher-test-{}", std::process::id()));
        std::fs::create_dir_all(&tmp_dir).unwrap();
        let config_path = tmp_dir.join("config.toml");

        // Write a valid config file.
        let mut f = std::fs::File::create(&config_path).unwrap();
        writeln!(f, "[window]\nwidth = 800.0\nheight = 600.0").unwrap();

        // Create watcher — should succeed.
        let (watcher, _rx) = ConfigWatcher::new(config_path.clone()).unwrap();

        // Drop watcher — should not panic.
        drop(watcher);

        // Cleanup.
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_config_watcher_detects_change() {
        let tmp_dir = std::env::temp_dir().join(format!(
            "crux-watcher-change-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&tmp_dir).unwrap();
        let config_path = tmp_dir.join("config.toml");

        // Write initial config.
        std::fs::write(&config_path, "[window]\nwidth = 800.0\nheight = 600.0\n").unwrap();

        let (_watcher, rx) = ConfigWatcher::new(config_path.clone()).unwrap();

        // Wait for the FSEvents watcher to fully settle before modifying.
        // macOS FSEvents can take up to ~1s to register a new watch.
        std::thread::sleep(Duration::from_millis(1000));
        std::fs::write(
            &config_path,
            "[window]\nwidth = 1024.0\nheight = 768.0\n",
        )
        .unwrap();

        // Wait for debounced event (500ms debounce + FSEvents latency).
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(ConfigEvent::Reloaded(boxed_config)) => {
                assert_eq!(boxed_config.window.width, 1024.0);
                assert_eq!(boxed_config.window.height, 768.0);
            }
            Ok(ConfigEvent::Error(e)) => panic!("Expected Reloaded, got Error: {}", e),
            Err(e) => panic!("Timed out waiting for config event: {}", e),
        }

        // Cleanup.
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_config_watcher_nonexistent_dir_created() {
        let tmp_dir = std::env::temp_dir().join(format!(
            "crux-watcher-nodir-test-{}/nested",
            std::process::id()
        ));
        let config_path = tmp_dir.join("config.toml");

        // Directory does not exist yet — watcher should create it.
        let result = ConfigWatcher::new(config_path);
        assert!(result.is_ok(), "Watcher should create missing directories");

        // Cleanup.
        let parent = std::env::temp_dir().join(format!(
            "crux-watcher-nodir-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&parent);
    }
}
