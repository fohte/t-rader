use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// SBI CSV preview の 1 行。
#[derive(Debug, Serialize, ToSchema)]
pub struct SbiPreviewRow {
    /// 元 CSV 上の行番号 (0-based)
    pub row_index: usize,
    pub date: NaiveDate,
    pub symbol: String,
    pub stock_name: String,
    /// "buy" | "sell"
    pub side: String,
    pub qty: Decimal,
    pub price: Decimal,
    pub fee: Decimal,
    /// 同日・同銘柄・同売買・同数量・同単価で既存取引が見つかったか
    pub is_duplicate: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SbiPreviewIssue {
    pub row_index: usize,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SbiPreviewResponse {
    pub rows: Vec<SbiPreviewRow>,
    pub issues: Vec<SbiPreviewIssue>,
}

/// SBI commit リクエストの 1 行。preview を確認後、行ごとに戦略 ID を割り当てる。
#[derive(Debug, PartialEq, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SbiCommitRow {
    pub strategy_id: Uuid,
    pub date: NaiveDate,
    pub symbol: String,
    #[serde(default)]
    pub stock_name: String,
    pub side: String,
    pub qty: Decimal,
    pub price: Decimal,
    #[serde(default)]
    pub fee: Option<Decimal>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SbiCommitRequest {
    pub rows: Vec<SbiCommitRow>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SbiCommitResponse {
    pub imported_count: usize,
    /// 重複検知でスキップした件数
    pub skipped_count: usize,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    // serde_json の arbitrary_precision feature は数値を一旦文字列として読み、u64 に
    // パースできない (= 小数を含む) 場合は visit_map 経路に落ちる。rust_decimal 側で
    // serde-arbitrary-precision feature を有効にしていないと DecimalVisitor::visit_map が
    // 未実装のままになり、小数を含む Decimal の deserialize だけが失敗する。
    #[test]
    fn commit_row_deserializes_fractional_price() {
        let json = indoc::indoc! {r#"
            {
                "strategy_id": "00000000-0000-0000-0000-000000000000",
                "date": "2026-01-15",
                "symbol": "7203",
                "side": "buy",
                "qty": 100,
                "price": 2500.5
            }
        "#};

        let row: SbiCommitRow = serde_json::from_str(json).unwrap();

        assert_eq!(
            row,
            SbiCommitRow {
                strategy_id: Uuid::nil(),
                date: NaiveDate::from_ymd_opt(2026, 1, 15).expect("valid date"),
                symbol: "7203".into(),
                stock_name: "".into(),
                side: "buy".into(),
                qty: Decimal::from_str("100").unwrap(),
                price: Decimal::from_str("2500.5").unwrap(),
                fee: None,
            }
        );
    }
}
