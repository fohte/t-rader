//! `/mcp/*` へのアクセスログ。
//!
//! rmcp の shadow stream 溢れ (WARN, "Shadow stream limit reached") は発生元の
//! IP/User-Agent/session を含まないため、ストームが起きても誰が叩いているか分からない。
//! ここでは HTTP レイヤーで送信元を独立に記録する。
//!
//! 毎リクエスト無条件に 1 行出すとストーム時に同じ問題 (大量ログによる圧迫) を
//! 再生産するため、同一 (path, client_ip) からの初回リクエストは即時に、
//! それ以降は [`LOG_INTERVAL`] ごとの集計 1 行にまとめて出す。

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

// ponytail: エントリを永久に保持する (evict しない)。path は 2 種類固定、
// client_ip も想定される呼び出し元は少数 (社内サービス + 上流コントロールプレーン) なので
// 実運用でメモリを圧迫する規模にはならない想定。無関係な送信元が大量に現れる構成になったら
// 古いエントリの TTL 削除を追加する。
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
                if elapsed >= LOG_INTERVAL {
                    let count = window.count;
                    window.window_start = now;
                    window.count = 0;
                    Some(LogEvent::WindowSummary { count, elapsed })
                } else {
                    window.count += 1;
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

/// クライアント IP を決定する。プロキシ経由なら `X-Forwarded-For` の先頭値、
/// なければ TCP 接続元 (`ConnectInfo`) を使う。
///
/// `ConnectInfo<SocketAddr>` は `OptionalFromRequestParts` を実装しておらず
/// `Option<ConnectInfo<SocketAddr>>` を直接 extractor として使えないため、
/// `Request::extensions()` から素で取り出す。
fn client_ip(request: &Request) -> String {
    if let Some(xff) = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
    {
        return xff.split(',').next().unwrap_or(xff).trim().to_string();
    }
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

    /// 初回リクエストは即時に First、窓の途中は None (集計のみ)、
    /// 窓を超えたら溜めていた件数を WindowSummary として吐き出す。
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
                count: 2,
                elapsed: flushed_at.duration_since(t0),
            })
        );

        // 集計後は新しい窓としてカウントし直す。
        assert_eq!(state.record(key("10.0.0.1"), flushed_at), None);
    }

    /// 別クライアント (path/ip の組が異なる) は独立に集計される。
    #[test]
    fn tracks_different_clients_independently() {
        let state = AccessLogState::new();
        let t0 = Instant::now();

        assert_eq!(state.record(key("10.0.0.1"), t0), Some(LogEvent::First));
        assert_eq!(state.record(key("10.0.0.2"), t0), Some(LogEvent::First));
    }
}
