//! 外部データ取得の証跡記録。
//!
//! `query_data` が取得したバーデータを `strategy_task_step_evidence` に保存し、
//! 「エージェントが当時何を見たか」を実行ステップ単位で再現可能にする。同じ問い合わせを
//! 後から実行しても、データプロバイダ側の更新により同じ結果が返るとは限らないため。
//!
//! `execution_step_id` に対応する `strategy_task_step` 行は t-rader-agent への polling
//! (`super::super::watcher`) で非同期に反映されるため、MCP tool 呼び出し時点ではまだ
//! 存在しないことがある (レース)。そのため FK は持たず、`note.execution_id` と同じく
//! 単なる相関用の UUID として扱う。

use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::entities::strategy_task_step_evidence;

use super::dto::BarDto;

/// snapshot に含めるバー件数の上限。日足で約 20 年分に相当し、通常の呼び出しでは
/// 到達しない安全弁。
const MAX_SNAPSHOT_BARS: usize = 5_000;

pub(super) async fn record_query_data(
    db: &DatabaseConnection,
    execution_step_id: Uuid,
    instrument_id: &str,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
    bars: &[BarDto],
) -> Result<(), sea_orm::DbErr> {
    let total_bars = bars.len();
    let truncated = total_bars > MAX_SNAPSHOT_BARS;
    let snapshot_bars = if truncated {
        &bars[total_bars - MAX_SNAPSHOT_BARS..]
    } else {
        bars
    };

    // 価格データには独立した公表時刻がないため、最新バーの時刻を両者に設定する。
    let last_ts = bars.last().map(|b| b.timestamp);

    let snapshot = serde_json::json!({
        "instrument_id": instrument_id,
        "from": from,
        "to": to,
        "bars": snapshot_bars,
        "total_bars": total_bars,
        "truncated": truncated,
    });

    strategy_task_step_evidence::ActiveModel {
        id: Set(Uuid::new_v4()),
        execution_step_id: Set(execution_step_id),
        source: Set("query_data".to_string()),
        source_ref: Set(instrument_id.to_string()),
        observed_at: Set(chrono::Utc::now().fixed_offset()),
        published_at: Set(last_ts),
        effective_at: Set(last_ts),
        snapshot: Set(snapshot),
    }
    .insert(db)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    use sqlx::PgPool;

    use crate::testing::create_test_db;

    use super::*;

    /// 基準日からの経過日数でタイムスタンプ昇順の `BarDto` を作る。
    fn bar_at(day_offset: i64) -> BarDto {
        let base =
            chrono::DateTime::parse_from_rfc3339("2025-01-01T00:00:00+00:00").expect("base ts");
        BarDto {
            timestamp: base + chrono::Duration::days(day_offset),
            open: 100.0,
            high: 110.0,
            low: 90.0,
            close: 105.0,
            volume: 1_000,
        }
    }

    #[sqlx::test(migrations = false)]
    async fn record_query_data_truncates_to_most_recent_bars_when_exceeding_max(pool: PgPool) {
        let db = create_test_db(pool).await;
        let execution_step_id = Uuid::new_v4();
        let total = MAX_SNAPSHOT_BARS + 1;
        let bars: Vec<BarDto> = (0..total as i64).map(bar_at).collect();
        let from = NaiveDate::from_ymd_opt(2025, 1, 1).expect("from");
        let to = NaiveDate::from_ymd_opt(2039, 1, 1).expect("to");

        record_query_data(&db, execution_step_id, "7203", from, to, &bars)
            .await
            .expect("record");

        let expected_bars = &bars[bars.len() - MAX_SNAPSHOT_BARS..];
        let last_ts = bars.last().map(|b| b.timestamp).expect("last bar");

        let rows = strategy_task_step_evidence::Entity::find()
            .filter(strategy_task_step_evidence::Column::ExecutionStepId.eq(execution_step_id))
            .all(&db)
            .await
            .expect("fetch evidence");
        assert_eq!(rows.len(), 1);
        let row = rows.into_iter().next().expect("row");
        let id = row.id;
        let observed_at = row.observed_at;

        assert_eq!(
            row,
            strategy_task_step_evidence::Model {
                id,
                execution_step_id,
                source: "query_data".to_string(),
                source_ref: "7203".to_string(),
                observed_at,
                published_at: Some(last_ts),
                effective_at: Some(last_ts),
                snapshot: serde_json::json!({
                    "instrument_id": "7203",
                    "from": from,
                    "to": to,
                    "bars": expected_bars,
                    "total_bars": total,
                    "truncated": true,
                }),
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn record_query_data_with_no_bars_leaves_published_and_effective_at_unset(pool: PgPool) {
        let db = create_test_db(pool).await;
        let execution_step_id = Uuid::new_v4();
        let bars: Vec<BarDto> = Vec::new();
        let from = NaiveDate::from_ymd_opt(2025, 1, 1).expect("from");
        let to = NaiveDate::from_ymd_opt(2025, 1, 2).expect("to");

        record_query_data(&db, execution_step_id, "7203", from, to, &bars)
            .await
            .expect("record");

        let rows = strategy_task_step_evidence::Entity::find()
            .filter(strategy_task_step_evidence::Column::ExecutionStepId.eq(execution_step_id))
            .all(&db)
            .await
            .expect("fetch evidence");
        assert_eq!(rows.len(), 1);
        let row = rows.into_iter().next().expect("row");
        let id = row.id;
        let observed_at = row.observed_at;

        assert_eq!(
            row,
            strategy_task_step_evidence::Model {
                id,
                execution_step_id,
                source: "query_data".to_string(),
                source_ref: "7203".to_string(),
                observed_at,
                published_at: None,
                effective_at: None,
                snapshot: serde_json::json!({
                    "instrument_id": "7203",
                    "from": from,
                    "to": to,
                    "bars": bars,
                    "total_bars": 0,
                    "truncated": false,
                }),
            },
        );
    }
}
