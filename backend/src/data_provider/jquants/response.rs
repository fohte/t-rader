use serde::Deserialize;

/// `data` + `pagination_key` を持つ一覧系レスポンスが実装するトレイト。
/// `fetch_all_pages` が `into_parts` でアイテム列と次ページキーを取り出す。
pub(crate) trait Paginated {
    type Item;

    fn into_parts(self) -> (Vec<Self::Item>, Option<String>);
}

/// J-Quants API V2 日足レスポンス (`GET /v2/equities/bars/daily`)
#[derive(Debug, Deserialize)]
pub(crate) struct DailyBarsResponse {
    pub data: Vec<DailyBar>,
    pub pagination_key: Option<String>,
}

impl Paginated for DailyBarsResponse {
    type Item = DailyBar;

    fn into_parts(self) -> (Vec<DailyBar>, Option<String>) {
        (self.data, self.pagination_key)
    }
}

/// J-Quants API V2 日足データ 1 レコード
///
/// 調整後価格 (AdjO 等) を使用する。未調整価格やセッション別データは無視する。
#[derive(Debug, Deserialize)]
pub(crate) struct DailyBar {
    #[serde(rename = "Date")]
    pub date: String,
    /// デシリアライズには必要だが、アプリ内部では fetch_daily_bars の引数 instrument_id を使う
    #[serde(rename = "Code")]
    pub _code: String,
    #[serde(rename = "AdjO")]
    pub adj_open: Option<f64>,
    #[serde(rename = "AdjH")]
    pub adj_high: Option<f64>,
    #[serde(rename = "AdjL")]
    pub adj_low: Option<f64>,
    #[serde(rename = "AdjC")]
    pub adj_close: Option<f64>,
    #[serde(rename = "AdjVo")]
    pub adj_volume: Option<f64>,
}

/// J-Quants API V2 銘柄マスタレスポンス (`GET /v2/equities/master`)
#[derive(Debug, Deserialize)]
pub(crate) struct EquitiesMasterResponse {
    pub data: Vec<EquityMaster>,
}

/// J-Quants API V2 銘柄マスタ 1 レコード
///
/// J-Quants は東証上場銘柄のみを提供するため、`MktNm` は構造体には含めない。
#[derive(Debug, Deserialize)]
pub(crate) struct EquityMaster {
    #[serde(rename = "Code")]
    pub code: String,
    #[serde(rename = "CoName")]
    pub company_name: String,
    #[serde(rename = "S33Nm")]
    pub sector_name: Option<String>,
}

/// J-Quants API V2 エラーレスポンス
#[derive(Debug, Deserialize)]
pub(crate) struct ErrorResponse {
    pub message: String,
}

/// J-Quants の「該当しない」値は空文字列で返る場合と JSON null で返る場合があり得るため、
/// number 型フィールドでもどちらも受理して `None` に正規化する。
///
/// この crate は `serde_json` を `arbitrary_precision` feature 付きで使っており、
/// この feature は `#[serde(untagged)]` enum による number 判定を壊すため、
/// 代わりに `serde_json::Value` を経由して型を判定する。
fn deserialize_optional_number<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<serde_json::Value>::deserialize(deserializer)? {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => n
            .as_f64()
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom(format!("number out of f64 range: {n}"))),
        Some(serde_json::Value::String(s)) if s.is_empty() => Ok(None),
        Some(other) => Err(serde::de::Error::custom(format!(
            "expected a number or empty string, got {other:?}"
        ))),
    }
}

/// J-Quants API V2 空売り残高報告レスポンス (`GET /v2/markets/short-sale-report`)
#[derive(Debug, Deserialize)]
pub(crate) struct ShortSaleReportResponse {
    pub data: Vec<ShortSaleReportRecord>,
    pub pagination_key: Option<String>,
}

impl Paginated for ShortSaleReportResponse {
    type Item = ShortSaleReportRecord;

    fn into_parts(self) -> (Vec<ShortSaleReportRecord>, Option<String>) {
        (self.data, self.pagination_key)
    }
}

/// J-Quants API V2 空売り残高報告 1 レコード
#[derive(Debug, Deserialize)]
pub(crate) struct ShortSaleReportRecord {
    #[serde(rename = "DiscDate")]
    pub disc_date: String,
    #[serde(rename = "CalcDate")]
    pub calc_date: String,
    #[serde(rename = "Code")]
    pub code: String,
    #[serde(rename = "SSName")]
    pub ss_name: String,
    #[serde(rename = "SSAddr")]
    pub ss_addr: String,
    #[serde(rename = "DICName")]
    pub dic_name: String,
    #[serde(rename = "DICAddr")]
    pub dic_addr: String,
    #[serde(rename = "FundName")]
    pub fund_name: String,
    #[serde(rename = "ShrtPosToSO")]
    pub short_position_ratio: f64,
    #[serde(rename = "ShrtPosShares")]
    pub short_position_shares: f64,
    #[serde(rename = "ShrtPosUnits")]
    pub short_position_units: f64,
    /// 該当なしは空文字列で返る (直近の報告がまだ無い等)
    #[serde(rename = "PrevRptDate")]
    pub prev_report_date: String,
    #[serde(
        rename = "PrevRptRatio",
        deserialize_with = "deserialize_optional_number",
        default
    )]
    pub prev_report_ratio: Option<f64>,
    #[serde(rename = "Notes")]
    pub notes: String,
}

/// J-Quants API V2 業種別空売り比率レスポンス (`GET /v2/markets/short-ratio`)
#[derive(Debug, Deserialize)]
pub(crate) struct ShortRatioResponse {
    pub data: Vec<ShortRatioRecord>,
    pub pagination_key: Option<String>,
}

impl Paginated for ShortRatioResponse {
    type Item = ShortRatioRecord;

    fn into_parts(self) -> (Vec<ShortRatioRecord>, Option<String>) {
        (self.data, self.pagination_key)
    }
}

/// J-Quants API V2 業種別空売り比率 1 レコード
#[derive(Debug, Deserialize)]
pub(crate) struct ShortRatioRecord {
    #[serde(rename = "Date")]
    pub date: String,
    /// 33 業種コード。`sector` テーブルとの対応付けは読む側に委ねる
    #[serde(rename = "S33")]
    pub s33: String,
    #[serde(
        rename = "SellExShortVa",
        deserialize_with = "deserialize_optional_number",
        default
    )]
    pub sell_excluding_short_value: Option<f64>,
    #[serde(
        rename = "ShrtWithResVa",
        deserialize_with = "deserialize_optional_number",
        default
    )]
    pub short_with_restriction_value: Option<f64>,
    #[serde(
        rename = "ShrtNoResVa",
        deserialize_with = "deserialize_optional_number",
        default
    )]
    pub short_without_restriction_value: Option<f64>,
}
