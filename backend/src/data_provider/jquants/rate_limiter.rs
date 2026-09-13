use std::collections::VecDeque;

use tokio::sync::Mutex;

/// レートリミットのウィンドウ幅 (60 秒)
pub(super) const RATE_LIMIT_WINDOW: std::time::Duration = std::time::Duration::from_secs(60);

/// スライディングウィンドウ方式のレートリミッター
///
/// 直近 60 秒間のリクエスト送信時刻を記録し、上限に達している場合は
/// 最も古いリクエストがウィンドウから外れるまで待機する。429 を受けた場合は
/// `note_rate_limited` により全呼び出しの送信を一定時間止める (cooldown)。
pub(super) struct RateLimiter {
    /// 直近のリクエスト送信時刻 (古い順)
    timestamps: Mutex<VecDeque<tokio::time::Instant>>,
    /// 429 受信時に設定される、送信を再開してよい時刻
    cooldown_until: Mutex<Option<tokio::time::Instant>>,
    /// 429 を受けてから送信を止める時間
    cooldown: std::time::Duration,
}

impl RateLimiter {
    pub(super) fn new(cooldown: std::time::Duration) -> Self {
        Self {
            timestamps: Mutex::new(VecDeque::new()),
            cooldown_until: Mutex::new(None),
            cooldown,
        }
    }

    /// 429 を受けたことを記録し、`cooldown` の間このクライアントの全呼び出しの送信を止める。
    pub(super) async fn note_rate_limited(&self) {
        *self.cooldown_until.lock().await = Some(tokio::time::Instant::now() + self.cooldown);
    }

    /// リクエスト送信の許可を取得する
    ///
    /// cooldown 中であればまずそれが明けるまで待つ。明けていれば、ウィンドウ内の
    /// リクエスト数が `max_requests` に達している場合、最も古いリクエストがウィンドウ
    /// から外れるまで待機する。`max_requests` は契約プランに応じて呼び出しごとに変わり
    /// うる (`JQuantsClient::current_rate_limit`)。
    pub(super) async fn acquire(&self, max_requests: usize) {
        loop {
            if let Some(until) = *self.cooldown_until.lock().await {
                let now = tokio::time::Instant::now();
                if now < until {
                    tracing::warn!(
                        wait_ms = until.saturating_duration_since(now).as_millis() as u64,
                        "429 の cooldown 中のため送信を停止して待機中"
                    );
                    tokio::time::sleep_until(until).await;
                    continue;
                }
            }

            let now = tokio::time::Instant::now();

            let mut timestamps = self.timestamps.lock().await;

            // ウィンドウ外のタイムスタンプを削除
            while let Some(&oldest) = timestamps.front() {
                if now.duration_since(oldest) >= RATE_LIMIT_WINDOW {
                    timestamps.pop_front();
                } else {
                    break;
                }
            }

            if timestamps.len() < max_requests {
                // 枠がある: タイムスタンプを記録して通過
                timestamps.push_back(now);
                return;
            }

            // 枠がない: 最も古いリクエストがウィンドウから外れるまで待つ
            let oldest = timestamps[0];
            let sleep_target = oldest + RATE_LIMIT_WINDOW;
            drop(timestamps); // ロックを解放してから sleep

            tracing::info!(
                max_requests,
                wait_ms = sleep_target.saturating_duration_since(now).as_millis() as u64,
                "レートリミットに到達、待機中"
            );
            tokio::time::sleep_until(sleep_target).await;
        }
    }
}
