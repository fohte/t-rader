//! `/mcp/*` へのアクセスログミドルウェア。
//!
//! 同一 (path, client_ip) からの初回リクエストは即時に、それ以降は
//! [`LOG_INTERVAL`] ごとの集計 1 行にまとめてログを出力する。

use std::collections::HashMap;
use std::collections::hash_map::Entry as MapEntry;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::HeaderMap;
use axum::http::header::{self, AsHeaderName};
use axum::middleware::Next;
use axum::response::Response;
use rmcp::transport::common::http_header::HEADER_SESSION_ID;

/// この間隔を超えて同一クライアントからのリクエストが続く場合、直近の窓の集計を 1 行出す。
const LOG_INTERVAL: Duration = Duration::from_secs(10);

#[derive(PartialEq, Eq, Hash)]
struct Key {
    path: String,
    client_ip: String,
}

struct WindowEntry {
    window_start: Instant,
    /// 直近の窓の開始以降、まだログに出していないリクエスト数。
    count: u64,
}

#[derive(Debug, PartialEq)]
enum LogEvent {
    /// このクライアントからの初回リクエスト。
    First,
    /// 直近の窓に溜まっていたリクエストの集計。
    WindowSummary { count: u64, elapsed: Duration },
}

// (path, client_ip) の組み合わせが少数かつ固定である前提のため、エントリの
// eviction は行わない。
#[derive(Clone, Default)]
pub struct AccessLogState(Arc<Mutex<HashMap<Key, WindowEntry>>>);

impl AccessLogState {
    pub fn new() -> Self {
        Self::default()
    }

    fn record(&self, key: Key, now: Instant) -> Option<LogEvent> {
        let mut entries = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match entries.entry(key) {
            MapEntry::Vacant(vacant) => {
                vacant.insert(WindowEntry {
                    window_start: now,
                    count: 0,
                });
                Some(LogEvent::First)
            }
            MapEntry::Occupied(mut occupied) => {
                let window = occupied.get_mut();
                let elapsed = now.duration_since(window.window_start);
                window.count += 1;
                if elapsed >= LOG_INTERVAL {
                    let count = window.count;
                    window.window_start = now;
                    window.count = 0;
                    Some(LogEvent::WindowSummary { count, elapsed })
                } else {
                    None
                }
            }
        }
    }
}

fn header_str(headers: &HeaderMap, name: impl AsHeaderName) -> String {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-")
        .to_string()
}

/// クライアント IP (TCP 接続元) を返す。`X-Forwarded-For` はクライアントが自由に
/// 詐称できるため使わない (集計キーに使うと evict しない [`AccessLogState`] を
/// 無制限に肥大化させられる)。
fn client_ip(request: &Request) -> String {
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip().to_string())
        .unwrap_or_else(|| "-".to_string())
}

pub async fn access_log(
    State(state): State<AccessLogState>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let client_ip = client_ip(&request);
    let headers = request.headers();
    let user_agent = header_str(headers, header::USER_AGENT);
    let mcp_session_id = header_str(headers, HEADER_SESSION_ID);

    let event = state.record(
        Key {
            path: path.clone(),
            client_ip: client_ip.clone(),
        },
        Instant::now(),
    );

    match event {
        Some(LogEvent::First) => tracing::info!(
            %method,
            %path,
            %client_ip,
            %user_agent,
            %mcp_session_id,
            "mcp access: first request from this client"
        ),
        Some(LogEvent::WindowSummary { count, elapsed }) => tracing::info!(
            %method,
            %path,
            %client_ip,
            %user_agent,
            %mcp_session_id,
            request_count = count,
            rate_per_sec = count as f64 / elapsed.as_secs_f64(),
            "mcp access: request rate summary"
        ),
        None => {}
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(client_ip: &str) -> Key {
        Key {
            path: "/mcp/strategy".to_string(),
            client_ip: client_ip.to_string(),
        }
    }

    #[test]
    fn tracks_first_request_then_flushes_window_summary_on_boundary() {
        let state = AccessLogState::new();
        let t0 = Instant::now();

        assert_eq!(state.record(key("10.0.0.1"), t0), Some(LogEvent::First));
        assert_eq!(
            state.record(key("10.0.0.1"), t0 + Duration::from_secs(1)),
            None
        );
        assert_eq!(
            state.record(key("10.0.0.1"), t0 + Duration::from_secs(5)),
            None
        );

        let flushed_at = t0 + LOG_INTERVAL + Duration::from_secs(1);
        assert_eq!(
            state.record(key("10.0.0.1"), flushed_at),
            Some(LogEvent::WindowSummary {
                count: 3,
                elapsed: flushed_at.duration_since(t0),
            })
        );

        assert_eq!(state.record(key("10.0.0.1"), flushed_at), None);
    }

    #[test]
    fn tracks_different_clients_independently() {
        let state = AccessLogState::new();
        let t0 = Instant::now();

        assert_eq!(state.record(key("10.0.0.1"), t0), Some(LogEvent::First));
        assert_eq!(state.record(key("10.0.0.2"), t0), Some(LogEvent::First));
    }
}
