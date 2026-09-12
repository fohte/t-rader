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
        &bars[..MAX_SNAPSHOT_BARS]
    } else {
        bars
    };

    // 価格データでは「公表時刻」と「有効時刻」が一致しうるため、両方とも最後のバーの
    // タイムスタンプを設定する。
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
