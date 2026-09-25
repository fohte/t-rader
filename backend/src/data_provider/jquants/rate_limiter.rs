use std::collections::VecDeque;

use tokio::sync::Mutex;

use crate::data_provider::DataProviderError;

/// レートリミットのウィンドウ幅 (60 秒)
pub(super) const RATE_LIMIT_WINDOW: std::time::Duration = std::time::Duration::from_secs(60);

/// スライディングウィンドウ方式のレートリミッター
///
/// 直近 60 秒間のリクエスト送信時刻を記録する。通常は枠が空くまで待機し、
/// テスト用の fail-fast 設定では枠超過をエラーにする。429 を受けた場合は
/// `note_rate_limited` により全呼び出しの送信を一定時間止める (cooldown)。
pub(super) struct RateLimiter {
    /// 直近のリクエスト送信時刻 (古い順)
    timestamps: Mutex<VecDeque<tokio::time::Instant>>,
    /// 429 受信時に設定される、送信を再開してよい時刻
    cooldown_until: Mutex<Option<tokio::time::Instant>>,
    /// 429 を受けてから送信を止める時間
    cooldown: std::time::Duration,
    /// window 枠が空くまで待つか、枠超過をエラーとして返すか
    #[cfg(test)]
    fail_fast_on_window_limit: bool,
}

impl RateLimiter {
    pub(super) fn new(cooldown: std::time::Duration) -> Self {
        Self {
            timestamps: Mutex::new(VecDeque::new()),
            cooldown_until: Mutex::new(None),
            cooldown,
            #[cfg(test)]
            fail_fast_on_window_limit: false,
        }
    }

    /// テスト用: window 枠が満杯になった時点で待機せずエラーを返す。
    #[cfg(test)]
    pub(super) fn new_fail_fast(cooldown: std::time::Duration) -> Self {
        Self {
            fail_fast_on_window_limit: true,
            ..Self::new(cooldown)
        }
    }

    /// 429 を受けたことを記録し、`cooldown` の間このクライアントの全呼び出しの送信を止める。
    pub(super) async fn note_rate_limited(&self) {
        *self.cooldown_until.lock().await = Some(tokio::time::Instant::now() + self.cooldown);
    }

    /// リクエスト送信の許可を取得する
    ///
    /// cooldown 中はまず明けるまで待つ。ウィンドウ内のリクエスト数が `max_requests` に
    /// 達した場合は、通常は最も古いリクエストが外れるまで待ち、fail-fast 設定ではエラーを返す。
    /// `max_requests` は契約プランに応じて呼び出しごとに変わりうる
    /// (`JQuantsClient::current_rate_limit`)。
    pub(super) async fn acquire(&self, max_requests: usize) -> Result<(), DataProviderError> {
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
                return Ok(());
            }

            #[cfg(test)]
            if self.fail_fast_on_window_limit {
                return Err(DataProviderError::RateLimitWindowFull { max_requests });
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
