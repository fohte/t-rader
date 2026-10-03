const DEFAULT_LOG_FILTER: &str = "info,sqlx=warn";

pub(super) fn default_log_filter() -> tracing_subscriber::EnvFilter {
    tracing_subscriber::EnvFilter::new(DEFAULT_LOG_FILTER)
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Clone)]
    struct LogBuffer(Arc<Mutex<Vec<u8>>>);

    struct LogBufferWriter(Arc<Mutex<Vec<u8>>>);

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogBuffer {
        type Writer = LogBufferWriter;

        fn make_writer(&'a self) -> Self::Writer {
            LogBufferWriter(Arc::clone(&self.0))
        }
    }

    impl Write for LogBufferWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .map_err(|_| io::Error::other("log buffer lock was poisoned"))?
                .extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn emitted_logs(filter: tracing_subscriber::EnvFilter) -> String {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .without_time()
            .with_ansi(false)
            .with_writer(LogBuffer(Arc::clone(&buffer)))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "sqlx::query", "query info");
            tracing::warn!(target: "sqlx::query", "query warning");
            tracing::info!(target: "backend::test", "application info");
        });

        let output = buffer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        String::from_utf8_lossy(&output).into_owned()
    }

    #[test]
    fn test_default_log_filter_suppresses_sqlx_info_and_keeps_warnings() {
        assert_eq!(
            emitted_logs(default_log_filter()),
            indoc::indoc!(
                "\
                \x20WARN sqlx::query: query warning
                \x20INFO backend::test: application info
                "
            ),
        );
    }
}
