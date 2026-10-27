const DEFAULT_LOG_FILTER: &str = "info,sqlx=warn";

pub(super) fn default_log_filter() -> tracing_subscriber::EnvFilter {
    tracing_subscriber::EnvFilter::new(DEFAULT_LOG_FILTER)
}

pub(super) fn build_subscriber<W>(
    filter: tracing_subscriber::EnvFilter,
    writer: W,
) -> impl tracing::Subscriber + Send + Sync + 'static
where
    W: for<'writer> tracing_subscriber::fmt::MakeWriter<'writer> + Send + Sync + 'static,
{
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        // Loki の logfmt parser は ANSI escape 付きの field key を解析できない。
        .with_ansi(false)
        .with_writer(writer)
        .finish()
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

    fn emitted_logs(filter: tracing_subscriber::EnvFilter, emit: impl FnOnce()) -> String {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = build_subscriber(filter, LogBuffer(Arc::clone(&buffer)));

        tracing::subscriber::with_default(subscriber, emit);

        let output = buffer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        String::from_utf8_lossy(&output).into_owned()
    }

    fn normalize_timestamps(logs: &str) -> String {
        logs.lines()
            .map(|line| {
                [" TRACE ", " DEBUG ", " INFO ", " WARN ", " ERROR "]
                    .into_iter()
                    .find_map(|level| line.find(level).map(|index| &line[index + 1..]))
                    .unwrap_or(line)
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    }

    #[test]
    fn test_default_log_filter_suppresses_sqlx_info_and_keeps_warnings() {
        assert_eq!(
            normalize_timestamps(&emitted_logs(default_log_filter(), || {
                tracing::info!(target: "sqlx::query", "query info");
                tracing::warn!(target: "sqlx::query", "query warning");
                tracing::info!(target: "backend::test", "application info");
            })),
            indoc::indoc!(
                "\
                 WARN sqlx::query: query warning
                 INFO backend::test: application info
                "
            ),
        );
    }

    #[test]
    fn test_subscriber_emits_logfmt_fields_without_ansi() {
        assert_eq!(
            normalize_timestamps(&emitted_logs(default_log_filter(), || {
                tracing::info!(target: "backend::test", event = "sample_event", sample_value = 17);
            })),
            indoc::indoc!(
                "\
                 INFO backend::test: event=\"sample_event\" sample_value=17
                "
            ),
        );
    }
}
