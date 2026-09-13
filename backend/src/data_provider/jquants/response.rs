use rust_decimal::Decimal;
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
    /// 商品区分コード (例: "011" = 内国株券、"014" = ETF)
    #[serde(rename = "ProdCat")]
    pub product_category: Option<String>,
}

/// J-Quants API V2 財務情報レスポンス (`GET /v2/fins/summary`)
///
/// フィールド数が多く記載欄も可変 (未記載の数値項目も空文字で返る等) なため、要素は
/// 個別の構造体にせず生の JSON のまま保持する。
#[derive(Debug, Deserialize)]
pub(crate) struct FinSummaryResponse {
    pub data: Vec<serde_json::Value>,
    pub pagination_key: Option<String>,
}

impl Paginated for FinSummaryResponse {
    type Item = serde_json::Value;

    fn into_parts(self) -> (Vec<serde_json::Value>, Option<String>) {
        (self.data, self.pagination_key)
    }
}

/// EDINET 由来のデータ (大量保有報告書 / 政策保有株式 / 大株主状況) の一覧レスポンス。
/// 書類ごとの内部構造はエンドポイントごとに異なるため、要素は serde_json::Value のまま保持する。
#[derive(Debug, Deserialize)]
pub(crate) struct EdinetDocumentsResponse {
    pub data: Vec<serde_json::Value>,
    pub pagination_key: Option<String>,
}

impl Paginated for EdinetDocumentsResponse {
    type Item = serde_json::Value;

    fn into_parts(self) -> (Vec<Self::Item>, Option<String>) {
        (self.data, self.pagination_key)
    }
}

/// J-Quants API V2 エラーレスポンス
#[derive(Debug, Deserialize)]
pub(crate) struct ErrorResponse {
    pub message: String,
}

/// J-Quants API V2 信用取引週末残高レスポンス (`GET /v2/markets/margin-interest`)
#[derive(Debug, Deserialize)]
pub(crate) struct MarginInterestResponse {
    pub data: Vec<MarginInterestApi>,
    pub pagination_key: Option<String>,
}

impl Paginated for MarginInterestResponse {
    type Item = MarginInterestApi;

    fn into_parts(self) -> (Vec<MarginInterestApi>, Option<String>) {
        (self.data, self.pagination_key)
    }
}

/// J-Quants API V2 信用取引週末残高 1 レコード
///
/// Val 系 (金額) フィールドは 2026-09-28 の新仕様切替 (2026-09-25 申込分以降) で追加され、
/// それ以前の日付では null になる。
#[derive(Debug, Deserialize)]
pub(crate) struct MarginInterestApi {
    #[serde(rename = "Date")]
    pub date: String,
    #[serde(rename = "Code")]
    pub code: String,
    #[serde(rename = "IssType")]
    pub iss_type: String,
    #[serde(rename = "ShrtVol")]
    pub shrt_vol: f64,
    #[serde(rename = "LongVol")]
    pub long_vol: f64,
    #[serde(rename = "ShrtNegVol")]
    pub shrt_neg_vol: f64,
    #[serde(rename = "LongNegVol")]
    pub long_neg_vol: f64,
    #[serde(rename = "ShrtStdVol")]
    pub shrt_std_vol: f64,
    #[serde(rename = "LongStdVol")]
    pub long_std_vol: f64,
    #[serde(rename = "ShrtVal")]
    pub shrt_val: Option<f64>,
    #[serde(rename = "LongVal")]
    pub long_val: Option<f64>,
    #[serde(rename = "ShrtNegVal")]
    pub shrt_neg_val: Option<f64>,
    #[serde(rename = "LongNegVal")]
    pub long_neg_val: Option<f64>,
    #[serde(rename = "ShrtStdVal")]
    pub shrt_std_val: Option<f64>,
    #[serde(rename = "LongStdVal")]
    pub long_std_val: Option<f64>,
}

/// J-Quants API V2 日々公表信用取引残高レスポンス (`GET /v2/markets/margin-alert`)
#[derive(Debug, Deserialize)]
pub(crate) struct MarginAlertResponse {
    pub data: Vec<MarginAlertApi>,
    pub pagination_key: Option<String>,
}

impl Paginated for MarginAlertResponse {
    type Item = MarginAlertApi;

    fn into_parts(self) -> (Vec<MarginAlertApi>, Option<String>) {
        (self.data, self.pagination_key)
    }
}

/// J-Quants API V2 日々公表信用取引残高 1 レコード
///
/// `*Chg` は前日に公表されていなければ `"-"`、`*Ratio`/`SLRatio` は ETF 等で `"*"` が入りうる。
///
/// これらのフィールドはいったん `serde_json::Value` で受け、`flexible_i64`/`flexible_decimal`
/// で変換する。`#[serde(untagged)]` な enum への `deserialize_with` は、この crate が有効化して
/// いる `arbitrary_precision` フィーチャーと組み合わせると数値のデシリアライズに失敗する
/// (untagged enum は `deserialize_any` でいったん内容をバッファするが、`arbitrary_precision`
/// 下では数値が専用の map 表現になり、カスタム enum の visitor がそれを解釈できないため)。
/// `serde_json::Value` はこの map 表現を正しく解釈できるため、`Value` を経由することで回避する。
#[derive(Debug, Deserialize)]
pub(crate) struct MarginAlertApi {
    #[serde(rename = "PubDate")]
    pub pub_date: String,
    #[serde(rename = "Code")]
    pub code: String,
    #[serde(rename = "AppDate")]
    pub app_date: String,
    #[serde(rename = "PubReason")]
    pub pub_reason: PubReasonApi,
    #[serde(rename = "ShrtOut")]
    pub shrt_out: f64,
    #[serde(rename = "LongOut")]
    pub long_out: f64,
    #[serde(rename = "ShrtOutChg")]
    pub shrt_out_chg: serde_json::Value,
    #[serde(rename = "LongOutChg")]
    pub long_out_chg: serde_json::Value,
    #[serde(rename = "ShrtOutRatio")]
    pub shrt_out_ratio: serde_json::Value,
    #[serde(rename = "LongOutRatio")]
    pub long_out_ratio: serde_json::Value,
    #[serde(rename = "SLRatio")]
    pub sl_ratio: serde_json::Value,
    #[serde(rename = "ShrtNegOut")]
    pub shrt_neg_out: f64,
    #[serde(rename = "ShrtStdOut")]
    pub shrt_std_out: f64,
    #[serde(rename = "LongNegOut")]
    pub long_neg_out: f64,
    #[serde(rename = "LongStdOut")]
    pub long_std_out: f64,
    #[serde(rename = "TSEMrgnRegCls")]
    pub tse_mrgn_reg_cls: String,
}

/// PubReason オブジェクト。各フィールドは "0"/"1" の文字列値で公表理由フラグを表す。
#[derive(Debug, Deserialize)]
pub(crate) struct PubReasonApi {
    #[serde(rename = "Restricted", deserialize_with = "deserialize_bool_flag")]
    pub restricted: bool,
    #[serde(
        rename = "DailyPublication",
        deserialize_with = "deserialize_bool_flag"
    )]
    pub daily_publication: bool,
    #[serde(rename = "Monitoring", deserialize_with = "deserialize_bool_flag")]
    pub monitoring: bool,
    #[serde(rename = "RestrictedByJSF", deserialize_with = "deserialize_bool_flag")]
    pub restricted_by_jsf: bool,
    #[serde(rename = "PrecautionByJSF", deserialize_with = "deserialize_bool_flag")]
    pub precaution_by_jsf: bool,
    #[serde(
        rename = "UnclearOrSecOnAlert",
        deserialize_with = "deserialize_bool_flag"
    )]
    pub unclear_or_sec_on_alert: bool,
}

/// "0"/"1" の文字列を bool にデシリアライズする ("1" のみ true)
fn deserialize_bool_flag<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(s == "1")
}

/// 数値または "-"/"*" 等の非数値文字列を表しうる JSON 値を Option<i64> に変換する。
/// 非数値は None として扱う (未公表・対象外を表す)。
pub(crate) fn flexible_i64(value: &serde_json::Value) -> Option<i64> {
    value.as_f64().map(|v| v.round() as i64)
}

/// 数値または "-"/"*" 等の非数値文字列を表しうる JSON 値を Option<Decimal> に変換する。
/// 非数値は None として扱う (未公表・対象外を表す)。
pub(crate) fn flexible_decimal(value: &serde_json::Value) -> Option<Decimal> {
    value.as_f64().and_then(|v| Decimal::try_from(v).ok())
}
