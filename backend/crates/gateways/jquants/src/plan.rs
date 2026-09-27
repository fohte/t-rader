use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};

const PREMIUM_HISTORY_DAYS: i64 = 365 * 20;

/// J-Quants の契約プラン
///
/// 各プランの配信遅延・提供期間は公式ドキュメント
/// (<https://jpx.gitbook.io/j-quants-ja/outline/data-spec>) のデータ提供期間に基づく。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JQuantsPlan {
    Free,
    Light,
    Standard,
    Premium,
}

impl JQuantsPlan {
    /// (配信遅延日数, 提供期間日数) を返す
    ///
    /// Light/Standard/Premium の配信遅延は公式ドキュメントに明記がないため、
    /// 遅延なし (0日) と仮定する。
    fn offsets(&self) -> (i64, i64) {
        match self {
            // 配信遅延 12週 (84日)、提供期間 2年 (730日)
            JQuantsPlan::Free => (84, 730),
            // 提供期間 5年 (1825日)
            JQuantsPlan::Light => (0, 1825),
            // 提供期間 10年 (3650日)
            JQuantsPlan::Standard => (0, 3650),
            JQuantsPlan::Premium => (0, PREMIUM_HISTORY_DAYS),
        }
    }

    /// `today` を基準に、このプランで取得可能な範囲 `(from, to)` を返す
    pub fn range(&self, today: NaiveDate) -> (NaiveDate, NaiveDate) {
        let (delay_days, history_days) = self.offsets();
        let to = today - Duration::days(delay_days);
        let from = to - Duration::days(history_days);
        (from, to)
    }

    /// このプランのレート制限 (1 分あたりのリクエスト数)
    ///
    /// 公式ページ (<https://jpx-jquants.com/ja/spec/rate-limits>) 記載の基本値。
    /// 「システムの状況等により調整される場合があります」との注記があり、
    /// エンドポイント別上限 (`/v2/fins/summary`, `/v2/fins/details` の 60) は未対応。
    pub fn rate_limit_per_minute(&self) -> usize {
        match self {
            JQuantsPlan::Free => 5,
            JQuantsPlan::Light => 60,
            JQuantsPlan::Standard => 120,
            JQuantsPlan::Premium => 500,
        }
    }
}

impl std::str::FromStr for JQuantsPlan {
    type Err = serde_json::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        serde_json::from_value(serde_json::Value::String(value.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::free(
        JQuantsPlan::Free,
        NaiveDate::from_ymd_opt(2026, 9, 13).expect("date"),
        (
            NaiveDate::from_ymd_opt(2024, 6, 21).expect("date"),
            NaiveDate::from_ymd_opt(2026, 6, 21).expect("date"),
        )
    )]
    #[case::light(
        JQuantsPlan::Light,
        NaiveDate::from_ymd_opt(2026, 9, 13).expect("date"),
        (
            NaiveDate::from_ymd_opt(2021, 9, 14).expect("date"),
            NaiveDate::from_ymd_opt(2026, 9, 13).expect("date"),
        )
    )]
    #[case::standard(
        JQuantsPlan::Standard,
        NaiveDate::from_ymd_opt(2026, 9, 13).expect("date"),
        (
            NaiveDate::from_ymd_opt(2016, 9, 15).expect("date"),
            NaiveDate::from_ymd_opt(2026, 9, 13).expect("date"),
        )
    )]
    #[case::premium(
        JQuantsPlan::Premium,
        NaiveDate::from_ymd_opt(2026, 9, 13).expect("date"),
        (
            NaiveDate::from_ymd_opt(2006, 9, 18).expect("date"),
            NaiveDate::from_ymd_opt(2026, 9, 13).expect("date"),
        )
    )]
    fn test_range(
        #[case] plan: JQuantsPlan,
        #[case] today: NaiveDate,
        #[case] expected: (NaiveDate, NaiveDate),
    ) {
        assert_eq!(plan.range(today), expected);
    }

    #[rstest]
    #[case::free(JQuantsPlan::Free, 5)]
    #[case::light(JQuantsPlan::Light, 60)]
    #[case::standard(JQuantsPlan::Standard, 120)]
    #[case::premium(JQuantsPlan::Premium, 500)]
    fn test_rate_limit_per_minute(#[case] plan: JQuantsPlan, #[case] expected: usize) {
        assert_eq!(plan.rate_limit_per_minute(), expected);
    }

    #[test]
    fn test_jquants_plan_serde_uses_snake_case() {
        assert_eq!(
            serde_json::to_value(JQuantsPlan::Standard).expect("serialize"),
            serde_json::json!("standard"),
        );
    }

    #[rstest]
    #[case::free("free", JQuantsPlan::Free)]
    #[case::light("light", JQuantsPlan::Light)]
    #[case::standard("standard", JQuantsPlan::Standard)]
    #[case::premium("premium", JQuantsPlan::Premium)]
    fn test_parse_plan(#[case] value: &str, #[case] expected: JQuantsPlan) {
        assert_eq!(value.parse::<JQuantsPlan>().ok(), Some(expected));
    }

    #[rstest]
    #[case::uppercase("Standard")]
    #[case::unknown("enterprise")]
    fn test_parse_plan_rejects_non_serde_values(#[case] value: &str) {
        assert!(value.parse::<JQuantsPlan>().is_err());
    }
}
