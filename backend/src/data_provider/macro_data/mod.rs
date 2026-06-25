pub mod stooq;
#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use utoipa::ToSchema;

use crate::data_provider::DataProviderError;

/// マクロ指標の現在値
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct MacroTick {
    /// 表示用シンボル名 (例: "日経225")
    pub symbol: String,
    /// 現在値 (フォーマット済み文字列)
    pub value: String,
    /// 前日終値からの変化率 (%)
    #[schema(value_type = f64)]
    pub pct: Decimal,
    /// このティックの取得時刻
    pub fetched_at: DateTime<Utc>,
}

/// マクロ指標 DataProvider の抽象 trait
#[async_trait::async_trait]
pub trait MacroDataProvider: Send + Sync {
    /// 現在値を取得する
    async fn fetch_macro_ticks(&self) -> Result<Vec<MacroTick>, DataProviderError>;
}

/// fetch 結果を保持する in-memory cache。
///
/// - 取得成功時: ticks を入れ替え、`stale_since = None`
/// - 取得失敗時: 既存 ticks を保持し、`stale_since` がまだ None なら現在時刻をセット
/// - 24h 以上経過した場合は handler 側で N/A 相当を返す
#[derive(Debug, Default)]
pub struct MacroCache {
    state: RwLock<MacroCacheState>,
}

#[derive(Debug, Clone, Default)]
struct MacroCacheState {
    ticks: Vec<MacroTick>,
    stale_since: Option<DateTime<Utc>>,
}

/// `MacroCache::snapshot` の戻り値
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroCacheSnapshot {
    pub ticks: Vec<MacroTick>,
    pub stale_since: Option<DateTime<Utc>>,
}

impl MacroCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// 取得成功を反映する
    pub async fn record_success(&self, ticks: Vec<MacroTick>) {
        let mut state = self.state.write().await;
        state.ticks = ticks;
        state.stale_since = None;
    }

    /// 取得失敗を反映する。`stale_since` が未設定なら `now` をセット
    pub async fn record_failure(&self, now: DateTime<Utc>) {
        let mut state = self.state.write().await;
        if state.stale_since.is_none() {
            state.stale_since = Some(now);
        }
    }

    /// 現在の cache 内容を返す
    pub async fn snapshot(&self) -> MacroCacheSnapshot {
        let state = self.state.read().await;
        MacroCacheSnapshot {
            ticks: state.ticks.clone(),
            stale_since: state.stale_since,
        }
    }
}

/// バックグラウンドで provider を一定間隔で poll し cache を更新する
///
/// 戻り値の `JoinHandle` を捨てるとランタイム停止時に task ごと終了する。
/// 1 回目は即実行し、その後 `interval` 間隔で繰り返す。
pub fn spawn_poll(
    provider: Arc<dyn MacroDataProvider>,
    cache: Arc<MacroCache>,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match provider.fetch_macro_ticks().await {
                Ok(ticks) => {
                    tracing::debug!(count = ticks.len(), "macro ticks fetched");
                    cache.record_success(ticks).await;
                }
                Err(err) => {
                    tracing::warn!(%err, "macro ticks fetch failed; keeping previous values");
                    cache.record_failure(Utc::now()).await;
                }
            }
        }
    })
}
