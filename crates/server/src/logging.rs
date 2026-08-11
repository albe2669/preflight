use std::path::Path;
use std::str::FromStr;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::filter::EnvFilter;

pub struct GuardedSubscriber {
    _guard: tracing_appender::non_blocking::WorkerGuard,
}

/// Build and install a JSON day-rolling file subscriber.
///
/// `level` is the LevelFilter parsed from the config (or env override).
/// `directory` is the log directory path.
///
/// RUST_LOG still takes precedence over the config level via EnvFilter builder.
pub fn init(level: LevelFilter, directory: &Path) -> anyhow::Result<GuardedSubscriber> {
    std::fs::create_dir_all(directory)
        .map_err(|e| anyhow::anyhow!("failed to create logging directory {directory:?}: {e}"))?;

    let env_filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();

    let appender = tracing_appender::rolling::daily(directory, "preflight.json.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(appender);

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(env_filter)
        .with_writer(non_blocking)
        .try_init()
        .map_err(|e| anyhow::anyhow!("failed to install tracing subscriber: {e}"))?;

    Ok(GuardedSubscriber { _guard })
}

/// Parse a level string from config or env and return the LevelFilter.
/// Returns an error for unrecognized values.
pub fn parse_level(s: &str) -> anyhow::Result<LevelFilter> {
    LevelFilter::from_str(s).map_err(|e| anyhow::anyhow!("invalid log level {s:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Write};
    use std::sync::{Arc, Mutex};
    use tracing::{debug, info};
    use tracing_subscriber::fmt::MakeWriter;
    /// Build a JSON subscriber with a custom writer (not installed globally).
    /// Used by tests to capture output.
    #[cfg(test)]
    pub fn build_subscriber<M>(
        level: LevelFilter,
        make_writer: M,
    ) -> impl tracing::Subscriber + Send + Sync + 'static
    where
        M: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
    {
        let env_filter = EnvFilter::builder()
            .with_default_directive(level.into())
            .from_env_lossy();

        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .with_writer(make_writer)
            .finish()
    }

    /// A writer that captures output into an Arc<Mutex<Vec<u8>>> for test assertions.
    struct CaptureWriter {
        buf: Arc<Mutex<Vec<u8>>>,
    }

    impl Write for CaptureWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.buf.lock().unwrap().write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for CaptureWriter {
        type Writer = CaptureWriter;

        fn make_writer(&self) -> Self::Writer {
            CaptureWriter {
                buf: Arc::clone(&self.buf),
            }
        }
    }

    #[test]
    fn test_json_output_contains_message_and_level() {
        let buf = Arc::new(Mutex::new(Vec::new()));
        let writer = CaptureWriter {
            buf: Arc::clone(&buf),
        };

        let subscriber = build_subscriber(LevelFilter::INFO, writer);
        let guard = tracing::subscriber::set_default(subscriber);
        info!("hello from test");

        drop(guard);
        let bytes = buf.lock().unwrap();
        let line = String::from_utf8_lossy(&bytes);

        // Should parse as JSON with "fields.message" and "level" fields
        let parsed: serde_json::Value =
            serde_json::from_str(line.trim()).expect("output should be valid JSON");
        let msg = parsed
            .get("fields")
            .and_then(|f| f.get("message"))
            .and_then(|v| v.as_str());
        assert_eq!(
            msg,
            Some("hello from test"),
            "JSON should contain the message field. Parsed: {parsed}"
        );
        assert_eq!(
            parsed.get("level").and_then(|v| v.as_str()),
            Some("INFO"),
            "JSON should contain the level field. Parsed: {parsed}"
        );
    }

    #[test]
    fn test_level_filtering_excludes_debug_at_info() {
        let buf = Arc::new(Mutex::new(Vec::new()));
        let writer = CaptureWriter {
            buf: Arc::clone(&buf),
        };

        let subscriber = build_subscriber(LevelFilter::INFO, writer);
        let guard = tracing::subscriber::set_default(subscriber);

        debug!("this should be filtered out");
        info!("this should be visible");

        drop(guard);
        let bytes = buf.lock().unwrap();
        let line = String::from_utf8_lossy(&bytes);

        // Should only contain the info message, not the debug one
        assert!(
            !line.contains("this should be filtered out"),
            "debug message should not appear at INFO level"
        );
        assert!(
            line.contains("this should be visible"),
            "info message should appear at INFO level"
        );
    }

    #[test]
    fn test_parse_level_invalid_returns_error() {
        let result = parse_level("not-a-level");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not-a-level"),
            "error should mention the invalid level: {err}"
        );
    }

    #[test]
    fn test_parse_level_valid_values() {
        assert!(parse_level("trace").is_ok());
        assert!(parse_level("debug").is_ok());
        assert!(parse_level("info").is_ok());
        assert!(parse_level("warn").is_ok());
        assert!(parse_level("error").is_ok());
        assert!(parse_level("off").is_ok());
        // Case insensitive
        assert!(parse_level("INFO").is_ok());
        assert!(parse_level("Debug").is_ok());
    }
}
