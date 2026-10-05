use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261005_141320_remove_fred_fomc_calendar_events"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "DELETE FROM calendar_event \
                 WHERE source = 'fred' AND external_id LIKE '101:%'",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // 誤った予定をロールバック時に再登録しない。
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "テスト準備の失敗時に詳細を表示するため"
    )]

    use sea_orm::sea_query::{Alias, Order, Query};
    use sea_orm::{ConnectionTrait, Database, TransactionTrait};
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    use super::Migration;

    #[tokio::test]
    async fn migration_removes_every_fred_release_101_event_and_preserves_other_sources() {
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let database = Database::connect(database_url)
            .await
            .expect("connect to test database");
        let transaction = database.begin().await.expect("begin transaction");
        let schema = format!("remove_fred_fomc_test_{}", std::process::id());
        transaction
            .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
            .await
            .expect("create isolated schema");
        transaction
            .execute_unprepared(&format!("SET LOCAL search_path TO {schema}"))
            .await
            .expect("set isolated schema");
        transaction
            .execute_unprepared(
                "CREATE TABLE calendar_event (
                    source text NOT NULL,
                    external_id text NOT NULL,
                    title text NOT NULL
                )",
            )
            .await
            .expect("create calendar event table");
        transaction
            .execute_unprepared(
                "INSERT INTO calendar_event (source, external_id, title) VALUES
                    ('fred', '101:2042-01-01', 'FOMC 声明'),
                    ('fred', '101:2043-01-01', 'FOMC 声明'),
                    ('fred', '10:2042-01-01', '架空指標'),
                    ('other', '101:2042-01-01', '架空イベント')",
            )
            .await
            .expect("insert calendar events");

        Migration
            .up(&SchemaManager::new(&transaction))
            .await
            .expect("run cleanup migration");

        let rows = transaction
            .query_all(
                &Query::select()
                    .column(Alias::new("source"))
                    .column(Alias::new("external_id"))
                    .column(Alias::new("title"))
                    .from(Alias::new("calendar_event"))
                    .order_by(Alias::new("source"), Order::Asc)
                    .order_by(Alias::new("external_id"), Order::Asc)
                    .to_owned(),
            )
            .await
            .expect("read remaining calendar events")
            .into_iter()
            .map(|row| {
                (
                    row.try_get::<String>("", "source").expect("read source"),
                    row.try_get::<String>("", "external_id")
                        .expect("read external id"),
                    row.try_get::<String>("", "title").expect("read title"),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            rows,
            vec![
                ("fred".into(), "10:2042-01-01".into(), "架空指標".into()),
                (
                    "other".into(),
                    "101:2042-01-01".into(),
                    "架空イベント".into(),
                ),
            ],
        );
        transaction.rollback().await.expect("roll back test schema");
    }
}
