//! 戦略実行 MCP の `read_margin` tool。信用残を銘柄・期間で読み出す。

use chrono::NaiveDate;
use core_application::margin::{MarginQuery, MarginUseCaseError};
use core_application::strategy_scope::StrategyScope;
use core_domain::margin::{MarginAlertRecord, MarginInterestRecord, PubReason};
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{StrategyServer, clamp_limit, decimal_to_f64, internal_error, invalid_params};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadMarginParams {
    /// 対象銘柄コード (4 桁、例: "1234")
    pub symbol: String,
    /// 取得開始日 (YYYY-MM-DD, inclusive)。省略時は下限なし
    pub from: Option<NaiveDate>,
    /// 取得終了日 (YYYY-MM-DD, inclusive)。省略時は上限なし
    pub to: Option<NaiveDate>,
    /// interest / alerts それぞれに独立に適用される件数上限
    pub limit: Option<u32>,
}

/// 信用取引週末残高 (2026-09-28 以降の切替後は日次の信用取引残高) 1 行分。
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct MarginInterestDto {
    pub date: NaiveDate,
    /// J-Quants の 5 桁コード。同一銘柄でも株式の種類ごとに別コードで並び得る
    pub code: String,
    /// 銘柄区分 (1: 信用銘柄, 2: 貸借銘柄, 3: その他)
    pub iss_type: i16,
    pub shrt_vol: i64,
    pub long_vol: i64,
    pub shrt_neg_vol: i64,
    pub long_neg_vol: i64,
    pub shrt_std_vol: i64,
    pub long_std_vol: i64,
    /// 新仕様開始前は金額データ自体が存在しないため null
    pub shrt_val: Option<i64>,
    pub long_val: Option<i64>,
    pub shrt_neg_val: Option<i64>,
    pub long_neg_val: Option<i64>,
    pub shrt_std_val: Option<i64>,
    pub long_std_val: Option<i64>,
}

impl From<MarginInterestRecord> for MarginInterestDto {
    fn from(record: MarginInterestRecord) -> Self {
        Self {
            date: record.date,
            code: record.code,
            iss_type: record.iss_type,
            shrt_vol: record.shrt_vol,
            long_vol: record.long_vol,
            shrt_neg_vol: record.shrt_neg_vol,
            long_neg_vol: record.long_neg_vol,
            shrt_std_vol: record.shrt_std_vol,
            long_std_vol: record.long_std_vol,
            shrt_val: record.shrt_val,
            long_val: record.long_val,
            shrt_neg_val: record.shrt_neg_val,
            long_neg_val: record.long_neg_val,
            shrt_std_val: record.shrt_std_val,
            long_std_val: record.long_std_val,
        }
    }
}

/// 日々公表信用取引残高 1 行分。取引所が日々公表銘柄に指定した銘柄のみが対象であり、
/// この一覧に載っていないことは残高ゼロを意味しない。
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct MarginAlertDto {
    /// 申込日。同一 app_date に訂正が複数あれば公表日 (pub_date) が最新の 1 件のみ返る
    pub app_date: NaiveDate,
    pub pub_date: NaiveDate,
    pub code: String,
    pub pub_reason: MarginPubReasonDto,
    pub shrt_out: i64,
    pub long_out: i64,
    /// 前日に公表されていなければ null
    pub shrt_out_chg: Option<i64>,
    pub long_out_chg: Option<i64>,
    /// ETF 等では null
    pub shrt_out_ratio: Option<f64>,
    pub long_out_ratio: Option<f64>,
    pub sl_ratio: Option<f64>,
    pub shrt_neg_out: i64,
    pub shrt_std_out: i64,
    pub long_neg_out: i64,
    pub long_std_out: i64,
    /// 規制区分 (文字列。J-Quants 側の分類をそのまま保持)
    pub tse_mrgn_reg_cls: String,
}

impl From<MarginAlertRecord> for MarginAlertDto {
    fn from(record: MarginAlertRecord) -> Self {
        Self {
            app_date: record.app_date,
            pub_date: record.pub_date,
            code: record.code,
            pub_reason: record.pub_reason.into(),
            shrt_out: record.shrt_out,
            long_out: record.long_out,
            shrt_out_chg: record.shrt_out_chg,
            long_out_chg: record.long_out_chg,
            shrt_out_ratio: record.shrt_out_ratio.map(decimal_to_f64),
            long_out_ratio: record.long_out_ratio.map(decimal_to_f64),
            sl_ratio: record.sl_ratio.map(decimal_to_f64),
            shrt_neg_out: record.shrt_neg_out,
            shrt_std_out: record.shrt_std_out,
            long_neg_out: record.long_neg_out,
            long_std_out: record.long_std_out,
            tse_mrgn_reg_cls: record.tse_mrgn_reg_cls,
        }
    }
}

/// 日々公表信用取引残高の公表理由フラグ。
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct MarginPubReasonDto {
    pub restricted: bool,
    pub daily_publication: bool,
    pub monitoring: bool,
    pub restricted_by_jsf: bool,
    pub precaution_by_jsf: bool,
    pub unclear_or_sec_on_alert: bool,
}

impl From<PubReason> for MarginPubReasonDto {
    fn from(reason: PubReason) -> Self {
        Self {
            restricted: reason.restricted,
            daily_publication: reason.daily_publication,
            monitoring: reason.monitoring,
            restricted_by_jsf: reason.restricted_by_jsf,
            precaution_by_jsf: reason.precaution_by_jsf,
            unclear_or_sec_on_alert: reason.unclear_or_sec_on_alert,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadMarginResult {
    /// 信用取引週末残高 (日次切替後は信用取引残高)。新しい順
    pub interest: Vec<MarginInterestDto>,
    /// 日々公表信用取引残高。新しい順
    pub alerts: Vec<MarginAlertDto>,
}

impl StrategyServer {
    pub(crate) async fn read_margin_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: ReadMarginParams,
    ) -> Result<ReadMarginResult, McpError> {
        let result = self
            .use_cases
            .margins
            .read(
                scope.into(),
                MarginQuery {
                    symbol: params.symbol,
                    from: params.from,
                    to: params.to,
                    limit: clamp_limit(params.limit),
                },
            )
            .await
            .map_err(margin_use_case_error)?;

        Ok(ReadMarginResult {
            interest: result.interest.into_iter().map(Into::into).collect(),
            alerts: result.alerts.into_iter().map(Into::into).collect(),
        })
    }
}

fn margin_use_case_error(error: MarginUseCaseError) -> McpError {
    match error {
        MarginUseCaseError::InvalidDateRange => invalid_params("from must be on or before to"),
        other => {
            tracing::error!(error = %other, "strategy mcp margin read failed");
            internal_error(format!("database error: {other}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MarginAlertDto, MarginInterestDto, MarginPubReasonDto, margin_use_case_error};
    use chrono::NaiveDate;
    use core_domain::margin::{MarginAlertRecord, MarginInterestRecord, PubReason};
    use rust_decimal::Decimal;

    fn ymd(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    #[test]
    fn maps_invalid_date_range_to_invalid_params() {
        let error =
            margin_use_case_error(core_application::margin::MarginUseCaseError::InvalidDateRange);

        assert_eq!(error.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[test]
    fn converts_margin_interest_to_dto() {
        let record = MarginInterestRecord {
            date: ymd(2024, 1, 2),
            code: "12340".into(),
            iss_type: 2,
            shrt_vol: 1,
            long_vol: 2,
            shrt_neg_vol: 3,
            long_neg_vol: 4,
            shrt_std_vol: 5,
            long_std_vol: 6,
            shrt_val: Some(7),
            long_val: Some(8),
            shrt_neg_val: Some(9),
            long_neg_val: Some(10),
            shrt_std_val: Some(11),
            long_std_val: Some(12),
        };

        assert_eq!(
            MarginInterestDto::from(record),
            MarginInterestDto {
                date: ymd(2024, 1, 2),
                code: "12340".into(),
                iss_type: 2,
                shrt_vol: 1,
                long_vol: 2,
                shrt_neg_vol: 3,
                long_neg_vol: 4,
                shrt_std_vol: 5,
                long_std_vol: 6,
                shrt_val: Some(7),
                long_val: Some(8),
                shrt_neg_val: Some(9),
                long_neg_val: Some(10),
                shrt_std_val: Some(11),
                long_std_val: Some(12),
            },
        );
    }

    #[test]
    fn converts_margin_alert_to_dto() {
        let record = MarginAlertRecord {
            pub_date: ymd(2024, 1, 3),
            code: "12340".into(),
            app_date: ymd(2024, 1, 2),
            pub_reason: PubReason {
                restricted: true,
                daily_publication: false,
                monitoring: true,
                restricted_by_jsf: false,
                precaution_by_jsf: true,
                unclear_or_sec_on_alert: false,
            },
            shrt_out: 1,
            long_out: 2,
            shrt_out_chg: Some(3),
            long_out_chg: None,
            shrt_out_ratio: Some(Decimal::new(4, 1)),
            long_out_ratio: None,
            sl_ratio: Some(Decimal::new(5, 1)),
            shrt_neg_out: 6,
            shrt_std_out: 7,
            long_neg_out: 8,
            long_std_out: 9,
            tse_mrgn_reg_cls: "001".into(),
        };

        assert_eq!(
            MarginAlertDto::from(record),
            MarginAlertDto {
                app_date: ymd(2024, 1, 2),
                pub_date: ymd(2024, 1, 3),
                code: "12340".into(),
                pub_reason: MarginPubReasonDto {
                    restricted: true,
                    daily_publication: false,
                    monitoring: true,
                    restricted_by_jsf: false,
                    precaution_by_jsf: true,
                    unclear_or_sec_on_alert: false,
                },
                shrt_out: 1,
                long_out: 2,
                shrt_out_chg: Some(3),
                long_out_chg: None,
                shrt_out_ratio: Some(0.4),
                long_out_ratio: None,
                sl_ratio: Some(0.5),
                shrt_neg_out: 6,
                shrt_std_out: 7,
                long_neg_out: 8,
                long_std_out: 9,
                tse_mrgn_reg_cls: "001".into(),
            },
        );
    }
}
