use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// J-Quants の契約プラン
///
/// 各プランの配信遅延・提供期間は公式ドキュメント
/// (<https://jpx.gitbook.io/j-quants-ja/outline/data-spec>) のデータ提供期間に基づく。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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
            // 提供期間は無制限相当のため、契約範囲未検出時のプローブ範囲と同じ値を使う
            JQuantsPlan::Premium => (0, crate::services::backfill::PROBE_MAX_HISTORY_DAYS),
        }
    }

    /// `today` を基準に、このプランで取得可能な範囲 `(from, to)` を返す
    pub fn range(&self, today: NaiveDate) -> (NaiveDate, NaiveDate) {
        let (delay_days, history_days) = self.offsets();
        let to = today - Duration::days(delay_days);
        let from = to - Duration::days(history_days);
        (from, to)
    }

    /// 検出された取得可能範囲から、最も近いプランを推定する。
    /// 範囲の日数 (to - from) と各プランの提供期間 (offsets().1) の差が最小のプランを選ぶ。
    pub fn infer_from_range(detected: (NaiveDate, NaiveDate)) -> JQuantsPlan {
        let observed_history_days = (detected.1 - detected.0).num_days().max(0);
        [
            JQuantsPlan::Free,
            JQuantsPlan::Light,
            JQuantsPlan::Standard,
            JQuantsPlan::Premium,
        ]
        .into_iter()
        .min_by_key(|p| (p.offsets().1 - observed_history_days).abs())
        .unwrap_or(JQuantsPlan::Standard)
    }
}

/// `jquants_plan_setting.plan_setting` JSONB に書き込む際の現行スキーマバージョン。
pub const JQUANTS_PLAN_SETTING_SCHEMA_VERSION: i32 = 1;

fn current_schema_version() -> i32 {
    JQUANTS_PLAN_SETTING_SCHEMA_VERSION
}

/// `jquants_plan_setting.plan_setting` の中身。`plan` が `None` の間は、
/// `JQuantsClient` の自動検出 (400 エラーからの契約範囲検出 + TTL) を使う。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct JQuantsPlanSettingData {
    #[serde(default = "current_schema_version")]
    pub schema_version: i32,
    #[serde(default)]
    pub plan: Option<JQuantsPlan>,
}

/// `plan_setting` JSONB カラムの値を型付きデータにパースする。
pub fn parse_plan_setting<T: serde::de::DeserializeOwned>(
    value: serde_json::Value,
) -> Result<T, crate::error::AppError> {
    serde_json::from_value(value).map_err(|e| {
        crate::error::AppError::Database(sea_orm::DbErr::Custom(format!(
            "invalid plan_setting: {e}"
        )))
    })
}

/// 型付きデータを `plan_setting` JSONB カラムに書き込む値へシリアライズする。
pub fn serialize_plan_setting<T: serde::Serialize>(
    data: &T,
) -> Result<serde_json::Value, crate::error::AppError> {
    serde_json::to_value(data).map_err(|e| {
        crate::error::AppError::Database(sea_orm::DbErr::Custom(format!(
            "invalid plan_setting: {e}"
        )))
    })
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PutJQuantsPlanSettingRequest {
    /// 手動設定するプラン。`null` で自動検出 (契約範囲の検出 + TTL) に戻す
    pub plan: Option<JQuantsPlan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct JQuantsFetchableRange {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct JQuantsPlanSettingResponse {
    pub plan: Option<JQuantsPlan>,
    /// 現在有効な取得可能範囲。`plan` が手動設定されていればそのプランの範囲、
    /// 未設定なら自動検出の結果 (未検出ならまだ null)。「未設定」は「何もしていない」
    /// ではなく「システムが自動で検出し設定している」ことを表すため、その結果を必ず返す。
    pub effective_range: Option<JQuantsFetchableRange>,
}

impl JQuantsPlanSettingResponse {
    pub fn new(
        data: JQuantsPlanSettingData,
        effective_range: Option<(NaiveDate, NaiveDate)>,
    ) -> Self {
        Self {
            plan: data.plan,
            effective_range: effective_range.map(|(from, to)| JQuantsFetchableRange { from, to }),
        }
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

    /// `from` を固定し、`history_days` 日後を `to` とした検出範囲を組み立てる
    fn detected_range(history_days: i64) -> (NaiveDate, NaiveDate) {
        let from = NaiveDate::from_ymd_opt(2020, 1, 1).expect("date");
        (from, from + Duration::days(history_days))
    }

    #[rstest]
    // 各プランの提供期間ちょうど
    #[case::exactly_free(730, JQuantsPlan::Free)]
    #[case::exactly_light(1825, JQuantsPlan::Light)]
    #[case::exactly_standard(3650, JQuantsPlan::Standard)]
    #[case::exactly_premium(
        crate::services::backfill::PROBE_MAX_HISTORY_DAYS,
        JQuantsPlan::Premium
    )]
    // Free/Light の中間 (1277.5 日) の前後
    #[case::free_light_boundary_rounds_down_to_free(1277, JQuantsPlan::Free)]
    #[case::free_light_boundary_rounds_up_to_light(1278, JQuantsPlan::Light)]
    // Light/Standard の中間 (2737.5 日) の前後
    #[case::light_standard_boundary_rounds_down_to_light(2737, JQuantsPlan::Light)]
    #[case::light_standard_boundary_rounds_up_to_standard(2738, JQuantsPlan::Standard)]
    // Standard/Premium の中間 (5475 日) はちょうど同点になるため、先に並ぶ Standard を選ぶ
    #[case::standard_premium_boundary_tie_prefers_standard(5475, JQuantsPlan::Standard)]
    // 極端に短い/長い範囲
    #[case::extremely_short_range(0, JQuantsPlan::Free)]
    #[case::extremely_long_range(100_000, JQuantsPlan::Premium)]
    fn test_infer_from_range(#[case] history_days: i64, #[case] expected: JQuantsPlan) {
        assert_eq!(
            JQuantsPlan::infer_from_range(detected_range(history_days)),
            expected
        );
    }

    #[test]
    fn test_infer_from_range_clamps_reversed_range_to_zero_days() {
        // to が from より前 (本来ありえないが、防御的に 0 日として扱う)
        let from = NaiveDate::from_ymd_opt(2020, 1, 1).expect("date");
        let to = from - Duration::days(10);
        assert_eq!(JQuantsPlan::infer_from_range((from, to)), JQuantsPlan::Free);
    }

    #[test]
    fn test_jquants_plan_setting_data_defaults_unknown_and_missing_fields() {
        let value: JQuantsPlanSettingData = serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "plan": "standard",
            "future_field": "x",
        }))
        .expect("parse");
        assert_eq!(
            value,
            JQuantsPlanSettingData {
                schema_version: 1,
                plan: Some(JQuantsPlan::Standard),
            }
        );

        let missing: JQuantsPlanSettingData =
            serde_json::from_value(serde_json::json!({})).expect("parse with defaults");
        assert_eq!(
            missing,
            JQuantsPlanSettingData {
                schema_version: JQUANTS_PLAN_SETTING_SCHEMA_VERSION,
                plan: None,
            }
        );
    }

    #[test]
    fn test_jquants_plan_serde_uses_snake_case() {
        assert_eq!(
            serde_json::to_value(JQuantsPlan::Standard).expect("serialize"),
            serde_json::json!("standard"),
        );
    }
}
