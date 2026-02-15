#[cfg(test)]
mod mock;

use chrono::NaiveDate;

use crate::models::{Bar, Instrument};

/// データプロバイダーで発生しうるエラー
#[derive(Debug, thiserror::Error)]
pub enum DataProviderError {
    /// 指定された銘柄が見つからない
    #[error("instrument not found: {0}")]
    NotFound(String),
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
}
