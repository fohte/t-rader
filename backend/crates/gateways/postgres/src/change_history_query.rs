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
    use core_application::change_history::{
        ChangeHistoryEntry, ChangeHistoryListQuery, ChangeHistoryQuery,
    };
    use rstest::{fixture, rstest};
    use sea_orm::{ActiveValue::Set, EntityTrait};
    use serde_json::json;
    use uuid::Uuid;

    use crate::DatabaseHandle;
    use crate::entities::change_history;
    use crate::test_support::create_test_transaction;

    use super::PostgresChangeHistoryQuery;

    fn timestamp(seconds: i64) -> DateTime<FixedOffset> {
        DateTime::<Utc>::UNIX_EPOCH
            .checked_add_signed(chrono::Duration::seconds(seconds))
            .expect("timestamp is in range")
            .fixed_offset()
    }

    async fn insert_history(db: &DatabaseHandle, history: &ChangeHistoryEntry) {
        change_history::Entity::insert(change_history::ActiveModel {
            id: Set(history.id),
            target_kind: Set(history.target_kind.clone()),
            target_id: Set(history.target_id),
            actor_kind: Set("human".into()),
            actor_label: Set("user".into()),
            op: Set("update".into()),
            diff_json: Set(history.diff_json.clone()),
            summary: Set(history.summary.clone()),
            created_at: Set(history.created_at),
        })
        .exec(db)
        .await
        .expect("insert test history");
    }

    struct HistoryFixture {
        target_id: Uuid,
        old: ChangeHistoryEntry,
        new: ChangeHistoryEntry,
        other_kind: ChangeHistoryEntry,
        other_target: ChangeHistoryEntry,
        missing_id: Uuid,
    }

    #[fixture]
    fn history_fixture() -> HistoryFixture {
        let target_id = Uuid::from_u128(10);
        HistoryFixture {
            target_id,
            old: entry(
                Uuid::from_u128(11),
                "trade",
                target_id,
                json!({ "revision": 1 }),
                None,
                timestamp(1),
            ),
            new: entry(
                Uuid::from_u128(12),
                "trade",
                target_id,
                json!({ "revision": 2 }),
                Some("Updated fixture"),
                timestamp(2),
            ),
            other_kind: entry(
                Uuid::from_u128(13),
                "note",
                target_id,
                json!({ "revision": 3 }),
                None,
                timestamp(3),
            ),
            other_target: entry(
                Uuid::from_u128(14),
                "trade",
                Uuid::from_u128(20),
                json!({ "revision": 4 }),
                None,
                timestamp(4),
            ),
            missing_id: Uuid::from_u128(15),
        }
    }

    async fn seed_history(db: &DatabaseHandle, fixture: &HistoryFixture) {
        for history in [
            &fixture.old,
            &fixture.new,
            &fixture.other_kind,
            &fixture.other_target,
        ] {
            insert_history(db, history).await;
        }
    }

    #[rstest]
    #[tokio::test]
    async fn list_orders_by_created_at_desc(history_fixture: HistoryFixture) {
        let db = create_test_transaction().await;
        seed_history(&db, &history_fixture).await;
        let query = PostgresChangeHistoryQuery::new(db);

        let result = query
            .list(ChangeHistoryListQuery {
                target_kind: None,
                target_id: None,
                limit: 10,
            })
            .await
            .map_err(|error| error.to_string());

        assert_eq!(
            result,
            Ok(vec![
                history_fixture.other_target,
                history_fixture.other_kind,
                history_fixture.new,
                history_fixture.old,
            ]),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn list_filters_by_target_kind_and_id(history_fixture: HistoryFixture) {
        let db = create_test_transaction().await;
        seed_history(&db, &history_fixture).await;
        let query = PostgresChangeHistoryQuery::new(db);

        let result = query
            .list(ChangeHistoryListQuery {
                target_kind: Some("trade".into()),
                target_id: Some(history_fixture.target_id),
                limit: 10,
            })
            .await
            .map_err(|error| error.to_string());

        assert_eq!(result, Ok(vec![history_fixture.new, history_fixture.old]),);
    }

    #[rstest]
    #[tokio::test]
    async fn find_by_id_returns_matching_history(history_fixture: HistoryFixture) {
        let db = create_test_transaction().await;
        seed_history(&db, &history_fixture).await;
        let query = PostgresChangeHistoryQuery::new(db);

        let result = query
            .find_by_id(history_fixture.new.id)
            .await
            .map_err(|error| error.to_string());

        assert_eq!(result, Ok(Some(history_fixture.new)));
    }

    #[rstest]
    #[tokio::test]
    async fn find_by_id_returns_none_for_missing_history(history_fixture: HistoryFixture) {
        let db = create_test_transaction().await;
        seed_history(&db, &history_fixture).await;
        let query = PostgresChangeHistoryQuery::new(db);

        let result = query
            .find_by_id(history_fixture.missing_id)
            .await
            .map_err(|error| error.to_string());

        assert_eq!(result, Ok(None));
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
