use sea_orm_migration::prelude::*;

pub struct Migration;

#[derive(DeriveIden)]
enum Trigger {
    Table,
    EventType,
}

#[derive(DeriveIden)]
enum Event {
    Table,
    Payload,
    ChainDepth,
    OccurredAt,
    DispatchedAt,
}

#[derive(DeriveIden)]
enum EventFiring {
    Table,
    EventId,
    TriggerId,
    TaskId,
}

#[derive(DeriveIden)]
enum StrategyTask {
    Table,
    TaskId,
}

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261015_121156_event_trigger_foundation"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Trigger::Table)
                    .add_column(ColumnDef::new(Trigger::EventType).text())
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE \"trigger\" DROP CONSTRAINT trigger_kind_shape_check; \
                 ALTER TABLE \"trigger\" ADD CONSTRAINT trigger_kind_shape_check CHECK (\
                    (kind = 'cron' AND schedule IS NOT NULL AND hook_slug IS NULL AND event_type IS NULL) \
                    OR (kind = 'hook' AND hook_slug IS NOT NULL AND schedule IS NULL AND event_type IS NULL) \
                    OR (kind = 'event' AND event_type ~ '^[a-z][a-z0-9_]*([.][a-z][a-z0-9_]*)+$' AND schedule IS NULL AND hook_slug IS NULL)\
                 )",
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Event::Table)
                    .col(
                        ColumnDef::new(Alias::new("event_id"))
                            .uuid()
                            .not_null()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(Alias::new("event_type")).text().not_null())
                    .col(ColumnDef::new(Event::Payload).json_binary().not_null())
                    .col(
                        ColumnDef::new(Event::ChainDepth)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Event::OccurredAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Event::DispatchedAt).timestamp_with_time_zone())
                    .primary_key(Index::create().col(Alias::new("event_id")).primary())
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE event ADD CONSTRAINT event_event_type_format_check \
                    CHECK (event_type ~ '^[a-z][a-z0-9_]*([.][a-z][a-z0-9_]*)+$'); \
                 ALTER TABLE event ADD CONSTRAINT event_payload_object_check \
                    CHECK (jsonb_typeof(payload) = 'object'); \
                 ALTER TABLE event ADD CONSTRAINT event_chain_depth_check CHECK (chain_depth >= 0)",
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(EventFiring::Table)
                    .col(ColumnDef::new(EventFiring::EventId).uuid().not_null())
                    .col(ColumnDef::new(EventFiring::TriggerId).uuid().not_null())
                    .col(
                        ColumnDef::new(EventFiring::TaskId)
                            .uuid()
                            .not_null()
                            .unique_key(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(EventFiring::Table, EventFiring::EventId)
                            .to(Event::Table, Alias::new("event_id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(EventFiring::Table, EventFiring::TriggerId)
                            .to(Trigger::Table, Alias::new("trigger_id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(EventFiring::Table, EventFiring::TaskId)
                            .to(StrategyTask::Table, StrategyTask::TaskId),
                    )
                    .primary_key(
                        Index::create()
                            .col(EventFiring::EventId)
                            .col(EventFiring::TriggerId)
                            .primary(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX event_pending_occurred_at_idx \
                     ON event (occurred_at) WHERE dispatched_at IS NULL; \
                 CREATE INDEX event_occurred_at_idx ON event (occurred_at); \
                 CREATE INDEX event_firing_trigger_id_idx ON event_firing (trigger_id); \
                 CREATE INDEX trigger_event_enabled_idx \
                     ON \"trigger\" (event_type) WHERE kind = 'event' AND enabled",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX trigger_event_enabled_idx")
            .await?;
        manager
            .drop_table(Table::drop().table(EventFiring::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Event::Table).to_owned())
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE \"trigger\" DROP CONSTRAINT trigger_kind_shape_check; \
                 ALTER TABLE \"trigger\" DROP COLUMN event_type; \
                 ALTER TABLE \"trigger\" ADD CONSTRAINT trigger_kind_shape_check CHECK (\
                    (kind = 'cron' AND schedule IS NOT NULL AND hook_slug IS NULL) \
                    OR (kind = 'hook' AND hook_slug IS NOT NULL AND schedule IS NULL)\
                 )",
            )
            .await?;

        Ok(())
    }
}
