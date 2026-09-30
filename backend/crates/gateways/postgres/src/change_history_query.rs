use async_trait::async_trait;
use core_application::change_history::{
    ChangeHistoryEntry, ChangeHistoryListQuery, ChangeHistoryQuery, ChangeHistoryQueryError,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::change_history;
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresChangeHistoryQuery {
    db: DatabaseHandle,
}

impl PostgresChangeHistoryQuery {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ChangeHistoryQuery for PostgresChangeHistoryQuery {
    async fn list(
        &self,
        query: ChangeHistoryListQuery,
    ) -> Result<Vec<ChangeHistoryEntry>, ChangeHistoryQueryError> {
        let mut select =
            change_history::Entity::find().order_by_desc(change_history::Column::CreatedAt);
        if let Some(target_kind) = query.target_kind {
            select = select.filter(change_history::Column::TargetKind.eq(target_kind));
        }
        if let Some(target_id) = query.target_id {
            select = select.filter(change_history::Column::TargetId.eq(target_id));
        }

        select
            .limit(query.limit)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(ChangeHistoryEntry::from).collect())
            .map_err(query_error)
    }

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<ChangeHistoryEntry>, ChangeHistoryQueryError> {
        change_history::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map(|row| row.map(ChangeHistoryEntry::from))
            .map_err(query_error)
    }
}

impl From<change_history::Model> for ChangeHistoryEntry {
    fn from(model: change_history::Model) -> Self {
        Self {
            id: model.id,
            target_kind: model.target_kind,
            target_id: model.target_id,
            actor_kind: model.actor_kind,
            actor_label: model.actor_label,
            op: model.op,
            diff_json: model.diff_json,
            summary: model.summary,
            created_at: model.created_at,
        }
    }
}

fn query_error(error: sea_orm::DbErr) -> ChangeHistoryQueryError {
    ChangeHistoryQueryError::Database(persistence_error(error))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "テストデータの構築失敗で即時にテストを失敗させるため"
    )]

    use chrono::{DateTime, FixedOffset, Utc};
    use core_application::change_history::{ChangeHistoryEntry, ChangeHistoryUseCases};
    use sea_orm::{ActiveValue::Set, EntityTrait};
    use serde_json::json;
    use uuid::Uuid;

    use crate::DatabaseHandle;
    use crate::entities::change_history;

    use super::PostgresChangeHistoryQuery;

    fn timestamp(seconds: i64) -> DateTime<FixedOffset> {
        DateTime::<Utc>::UNIX_EPOCH
            .checked_add_signed(chrono::Duration::seconds(seconds))
            .expect("timestamp is in range")
            .fixed_offset()
    }

    async fn insert_history(
        db: &DatabaseHandle,
        id: Uuid,
        target_kind: &str,
        target_id: Uuid,
        diff_json: serde_json::Value,
        summary: Option<&str>,
        created_at: DateTime<FixedOffset>,
    ) {
        change_history::Entity::insert(change_history::ActiveModel {
            id: Set(id),
            target_kind: Set(target_kind.into()),
            target_id: Set(target_id),
            actor_kind: Set("human".into()),
            actor_label: Set("user".into()),
            op: Set("update".into()),
            diff_json: Set(diff_json),
            summary: Set(summary.map(str::to_owned)),
            created_at: Set(created_at),
        })
        .exec(db)
        .await
        .expect("insert test history");
    }

    #[backend_test_macros::database_test]
    async fn query_reads_history_with_filters_order_limits_and_missing_ids(db: DatabaseHandle) {
        let target_id = Uuid::from_u128(10);
        let old_id = Uuid::from_u128(11);
        let new_id = Uuid::from_u128(12);
        let unrelated_id = Uuid::from_u128(13);
        let missing_id = Uuid::from_u128(14);
        insert_history(
            &db,
            old_id,
            "trade",
            target_id,
            json!({ "revision": 1 }),
            None,
            timestamp(1),
        )
        .await;
        insert_history(
            &db,
            new_id,
            "trade",
            target_id,
            json!({ "revision": 2 }),
            Some("Updated fixture"),
            timestamp(2),
        )
        .await;
        insert_history(
            &db,
            unrelated_id,
            "note",
            Uuid::from_u128(20),
            json!({ "revision": 3 }),
            None,
            timestamp(3),
        )
        .await;

        let use_cases =
            ChangeHistoryUseCases::new(std::sync::Arc::new(PostgresChangeHistoryQuery::new(db)));
        let all = use_cases
            .list(Some(""), None, None)
            .await
            .map_err(|error| error.to_string());
        let filtered = use_cases
            .list(Some("trade"), Some(target_id), Some(0))
            .await
            .map_err(|error| error.to_string());
        let detail = use_cases
            .get(new_id)
            .await
            .map_err(|error| error.to_string());
        let missing = use_cases
            .get(missing_id)
            .await
            .map_err(|error| error.to_string());

        assert_eq!(
            (all, filtered, detail, missing),
            (
                Ok(vec![
                    entry(
                        unrelated_id,
                        "note",
                        Uuid::from_u128(20),
                        json!({ "revision": 3 }),
                        None,
                        timestamp(3),
                    ),
                    entry(
                        new_id,
                        "trade",
                        target_id,
                        json!({ "revision": 2 }),
                        Some("Updated fixture"),
                        timestamp(2),
                    ),
                    entry(
                        old_id,
                        "trade",
                        target_id,
                        json!({ "revision": 1 }),
                        None,
                        timestamp(1),
                    ),
                ]),
                Ok(vec![entry(
                    new_id,
                    "trade",
                    target_id,
                    json!({ "revision": 2 }),
                    Some("Updated fixture"),
                    timestamp(2),
                )]),
                Ok(entry(
                    new_id,
                    "trade",
                    target_id,
                    json!({ "revision": 2 }),
                    Some("Updated fixture"),
                    timestamp(2),
                )),
                Err(format!("history {missing_id} not found")),
            ),
        );
    }

    fn entry(
        id: Uuid,
        target_kind: &str,
        target_id: Uuid,
        diff_json: serde_json::Value,
        summary: Option<&str>,
        created_at: DateTime<FixedOffset>,
    ) -> ChangeHistoryEntry {
        ChangeHistoryEntry {
            id,
            target_kind: target_kind.into(),
            target_id,
            actor_kind: "human".into(),
            actor_label: "user".into(),
            op: "update".into(),
            diff_json,
            summary: summary.map(str::to_owned),
            created_at,
        }
    }
}
