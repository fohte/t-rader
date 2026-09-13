pub mod ibkr;
pub mod jquants;
pub mod macro_data;
#[cfg(test)]
mod mock;
pub mod news;

use chrono::NaiveDate;

use crate::models::{Bar, Instrument};
use ibkr::IbkrClient;
use jquants::JQuantsClient;

/// データプロバイダーで発生しうるエラー
#[derive(Debug, thiserror::Error)]
pub enum DataProviderError {
    /// 指定された銘柄が見つからない
    #[error("instrument not found: {0}")]
    NotFound(String),

    /// ネットワーク通信エラー (接続失敗、タイムアウト等)
    #[error("network error: {0}")]
    Network(String),

    /// API がエラーレスポンスを返した (400, 403 等)
    #[error("api error (status {status}): {message}")]
    Api { status: u16, message: String },

    /// レートリミット超過でリトライ上限に到達
    #[error("rate limited after {retries} retries")]
    RateLimited { retries: u32 },

    /// レスポンスのパースに失敗
    #[error("failed to parse response: {0}")]
    Parse(String),

    /// データプロバイダー内部の DB アクセスに失敗 (集約結果の永続化など)
    #[error("database error: {0}")]
    Database(String),
}

/// 日足データの取得期間を指定するパラメータ
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateRange {
    /// 取得開始日 (この日を含む)
    pub from: NaiveDate,
    /// 取得終了日 (この日を含む)
    pub to: NaiveDate,
}

/// 株価データプロバイダーの抽象化 trait
///
/// 日足 OHLCV データや銘柄情報の取得元を差し替え可能にする。
/// Axum のハンドラから使用するため Send + Sync を要求する。
/// crate 内でのみ使用するため async fn in trait の auto trait bounds は問題にならない。
#[expect(async_fn_in_trait, reason = "crate 内でのみ使用する trait のため")]
pub trait DataProvider: Send + Sync {
    /// 指定銘柄・期間の日足バーデータを取得する
    ///
    /// 戻り値のバーはタイムスタンプ昇順でソートされる。
    /// 該当データが存在しない場合は空の Vec を返す。
    async fn fetch_daily_bars(
        &self,
        instrument_id: &str,
        range: &DateRange,
    ) -> Result<Vec<Bar>, DataProviderError>;

    /// 指定銘柄の情報を取得する
    async fn fetch_instrument(&self, instrument_id: &str) -> Result<Instrument, DataProviderError>;

    /// 把握している契約範囲 (取得可能な最古日・最新日)。把握する仕組みを持たないプロバイダは
    /// 常に `None` を返す。
    fn known_fetchable_range(&self) -> Option<(NaiveDate, NaiveDate)> {
        None
    }

    /// 検出済みの契約範囲を、まだ手動設定 (推定含む) が無ければ初回のみプランとして推定し、
    /// 永続化する。対応するプロバイダ (J-Quants) のみ意味のある実装を持ち、デフォルトは no-op。
    /// 「継続的に自動追従し続ける」のではなく「未設定の間に一度だけ推定して固定する」ための
    /// フック。
    async fn persist_inferred_range_if_needed(
        &self,
        _db: &sea_orm::DatabaseConnection,
    ) -> Result<(), DataProviderError> {
        Ok(())
    }
}

/// DataProvider の具体的な実装を列挙する enum
///
/// async fn in trait は dyn 互換でないため、`Arc<dyn DataProvider>` の代わりに
/// enum ディスパッチでポリモーフィズムを実現する。
pub enum DataProviderKind {
    JQuants(JQuantsClient),
    Ibkr(IbkrClient),
}

impl DataProvider for DataProviderKind {
    async fn fetch_daily_bars(
        &self,
        instrument_id: &str,
        range: &DateRange,
    ) -> Result<Vec<Bar>, DataProviderError> {
        match self {
            DataProviderKind::JQuants(client) => {
                client.fetch_daily_bars(instrument_id, range).await
            }
            DataProviderKind::Ibkr(client) => client.fetch_daily_bars(instrument_id, range).await,
        }
    }

    async fn fetch_instrument(&self, instrument_id: &str) -> Result<Instrument, DataProviderError> {
        match self {
            DataProviderKind::JQuants(client) => client.fetch_instrument(instrument_id).await,
            DataProviderKind::Ibkr(client) => client.fetch_instrument(instrument_id).await,
        }
    }

    fn known_fetchable_range(&self) -> Option<(NaiveDate, NaiveDate)> {
        match self {
            DataProviderKind::JQuants(client) => client.known_fetchable_range(),
            DataProviderKind::Ibkr(client) => client.known_fetchable_range(),
        }
    }

    async fn persist_inferred_range_if_needed(
        &self,
        db: &sea_orm::DatabaseConnection,
    ) -> Result<(), DataProviderError> {
        match self {
            DataProviderKind::JQuants(client) => client.persist_inferred_range_if_needed(db).await,
            DataProviderKind::Ibkr(client) => client.persist_inferred_range_if_needed(db).await,
        }
    }
}
