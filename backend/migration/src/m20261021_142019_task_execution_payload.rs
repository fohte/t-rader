use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261021_142019_task_execution_payload"
    }
}

#[derive(DeriveIden)]
enum StrategyTask {
    Table,
    TaskId,
}

#[derive(DeriveIden)]
enum TaskExecutionDefinitionSnapshot {
    Table,
    TaskId,
    DefinitionKey,
    Definition,
    Digest,
    CreatedAt,
}

#[derive(DeriveIden)]
enum TaskExecutionPayload {
    Table,
    Id,
    TaskId,
    ExecutionStepId,
    Kind,
    Source,
    SourceRevision,
    SourceLocator,
    SourceMetadata,
    Schema,
    RootIsArray,
    ChunkCount,
    ItemCount,
    ByteCount,
    Digest,
    Status,
    IncompleteReason,
    CreatedAt,
    SealedAt,
}

#[derive(DeriveIden)]
enum TaskExecutionPayloadChunk {
    Table,
    PayloadId,
    Sequence,
    StartItem,
    Data,
    ByteCount,
    ItemCount,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TaskExecutionDefinitionSnapshot::Table)
                    .col(
                        ColumnDef::new(TaskExecutionDefinitionSnapshot::TaskId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionDefinitionSnapshot::DefinitionKey)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionDefinitionSnapshot::Definition)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionDefinitionSnapshot::Digest)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionDefinitionSnapshot::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(TaskExecutionDefinitionSnapshot::TaskId)
                            .col(TaskExecutionDefinitionSnapshot::DefinitionKey),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                TaskExecutionDefinitionSnapshot::Table,
                                TaskExecutionDefinitionSnapshot::TaskId,
                            )
                            .to(StrategyTask::Table, StrategyTask::TaskId)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TaskExecutionPayload::Table)
                    .col(
                        ColumnDef::new(TaskExecutionPayload::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TaskExecutionPayload::TaskId).uuid().not_null())
                    .col(ColumnDef::new(TaskExecutionPayload::ExecutionStepId).uuid())
                    .col(ColumnDef::new(TaskExecutionPayload::Kind).text().not_null())
                    .col(ColumnDef::new(TaskExecutionPayload::Source).text().not_null())
                    .col(ColumnDef::new(TaskExecutionPayload::SourceRevision).text())
                    .col(ColumnDef::new(TaskExecutionPayload::SourceLocator).json_binary())
                    .col(
                        ColumnDef::new(TaskExecutionPayload::SourceMetadata)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionPayload::Schema)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionPayload::RootIsArray)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionPayload::ChunkCount)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionPayload::ItemCount)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionPayload::ByteCount)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(TaskExecutionPayload::Digest).text())
                    .col(
                        ColumnDef::new(TaskExecutionPayload::Status)
                            .text()
                            .not_null()
                            .default("staging"),
                    )
                    .col(ColumnDef::new(TaskExecutionPayload::IncompleteReason).text())
                    .col(
                        ColumnDef::new(TaskExecutionPayload::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(TaskExecutionPayload::SealedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .from(TaskExecutionPayload::Table, TaskExecutionPayload::TaskId)
                            .to(StrategyTask::Table, StrategyTask::TaskId)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TaskExecutionPayloadChunk::Table)
                    .col(
                        ColumnDef::new(TaskExecutionPayloadChunk::PayloadId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionPayloadChunk::Sequence)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionPayloadChunk::StartItem)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionPayloadChunk::Data)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionPayloadChunk::ByteCount)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskExecutionPayloadChunk::ItemCount)
                            .big_integer()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(TaskExecutionPayloadChunk::PayloadId)
                            .col(TaskExecutionPayloadChunk::Sequence),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TaskExecutionPayloadChunk::Table, TaskExecutionPayloadChunk::PayloadId)
                            .to(TaskExecutionPayload::Table, TaskExecutionPayload::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE task_execution_payload \
                 ADD CONSTRAINT task_execution_payload_kind_check \
                 CHECK (kind IN ('input', 'output')); \
                 ALTER TABLE task_execution_payload \
                 ADD CONSTRAINT task_execution_payload_status_check \
                 CHECK (status IN ('staging', 'sealed', 'incomplete')); \
                 ALTER TABLE task_execution_payload \
                 ADD CONSTRAINT task_execution_payload_counts_check \
                 CHECK (chunk_count >= 0 AND item_count >= 0 AND byte_count >= 0); \
                 ALTER TABLE task_execution_payload_chunk \
                 ADD CONSTRAINT task_execution_payload_chunk_counts_check \
                 CHECK (sequence >= 0 AND start_item >= 0 AND byte_count >= 0 AND item_count >= 0); \
                 ALTER TABLE task_execution_payload \
                 ADD CONSTRAINT task_execution_payload_sealed_check \
                 CHECK ((status = 'sealed' AND digest IS NOT NULL AND sealed_at IS NOT NULL) \
                     OR (status <> 'sealed' AND digest IS NULL AND sealed_at IS NULL)); \
                 CREATE INDEX task_execution_payload_task_created_idx \
                     ON task_execution_payload (task_id, created_at, id); \
                 CREATE INDEX task_execution_payload_chunk_range_idx \
                     ON task_execution_payload_chunk (payload_id, start_item); \
                 CREATE OR REPLACE FUNCTION reject_execution_definition_snapshot_change() \
                 RETURNS trigger LANGUAGE plpgsql AS $$ \
                 BEGIN \
                     RAISE EXCEPTION 'execution definition snapshots are immutable'; \
                 END; \
                 $$; \
                 CREATE TRIGGER task_execution_definition_snapshot_immutable \
                     BEFORE UPDATE OR DELETE ON task_execution_definition_snapshot \
                     FOR EACH ROW EXECUTE FUNCTION reject_execution_definition_snapshot_change(); \
                 CREATE OR REPLACE FUNCTION protect_execution_payload() \
                 RETURNS trigger LANGUAGE plpgsql AS $$ \
                 BEGIN \
                     IF TG_OP = 'DELETE' THEN \
                         RAISE EXCEPTION 'execution payloads are immutable'; \
                     END IF; \
                     IF OLD.status <> 'staging' THEN \
                         RAISE EXCEPTION 'sealed or incomplete execution payloads are immutable'; \
                     END IF; \
                     IF NEW.id <> OLD.id OR NEW.task_id <> OLD.task_id \
                        OR NEW.execution_step_id IS DISTINCT FROM OLD.execution_step_id \
                        OR NEW.kind <> OLD.kind OR NEW.source <> OLD.source \
                        OR NEW.source_revision IS DISTINCT FROM OLD.source_revision \
                        OR NEW.source_locator IS DISTINCT FROM OLD.source_locator \
                        OR NEW.source_metadata <> OLD.source_metadata OR NEW.schema <> OLD.schema \
                        OR NEW.root_is_array <> OLD.root_is_array OR NEW.created_at <> OLD.created_at THEN \
                         RAISE EXCEPTION 'execution payload identity is immutable'; \
                     END IF; \
                     RETURN NEW; \
                 END; \
                 $$; \
                 CREATE TRIGGER task_execution_payload_immutable \
                     BEFORE UPDATE OR DELETE ON task_execution_payload \
                     FOR EACH ROW EXECUTE FUNCTION protect_execution_payload(); \
                 CREATE OR REPLACE FUNCTION protect_execution_payload_chunk() \
                 RETURNS trigger LANGUAGE plpgsql AS $$ \
                 DECLARE payload_status text; payload_item_count bigint; payload_chunk_count bigint; \
                 BEGIN \
                     IF TG_OP <> 'INSERT' THEN \
                         RAISE EXCEPTION 'execution payload chunks are immutable'; \
                     END IF; \
                     SELECT status, item_count, chunk_count \
                         INTO payload_status, payload_item_count, payload_chunk_count \
                         FROM task_execution_payload \
                         WHERE id = NEW.payload_id FOR UPDATE; \
                     IF payload_status IS DISTINCT FROM 'staging' THEN \
                         RAISE EXCEPTION 'execution payload is not staging'; \
                     END IF; \
                     IF NEW.sequence <> payload_chunk_count THEN \
                         RAISE EXCEPTION 'execution payload chunk sequence is not contiguous'; \
                     END IF; \
                     IF NEW.start_item <> payload_item_count THEN \
                         RAISE EXCEPTION 'execution payload chunk range is not contiguous'; \
                     END IF; \
                     RETURN NEW; \
                 END; \
                 $$; \
                 CREATE TRIGGER task_execution_payload_chunk_staging_only \
                     BEFORE INSERT OR UPDATE OR DELETE ON task_execution_payload_chunk \
                     FOR EACH ROW EXECUTE FUNCTION protect_execution_payload_chunk(); \
                 ALTER TABLE custom_indicator_evaluation \
                     ADD COLUMN payload_id uuid, \
                     ADD COLUMN payload_pointers jsonb, \
                     ADD COLUMN execution_step_id uuid, \
                     ADD CONSTRAINT custom_indicator_evaluation_payload_fk \
                         FOREIGN KEY (payload_id) REFERENCES task_execution_payload(id) ON DELETE RESTRICT;",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE custom_indicator_evaluation \
                     DROP CONSTRAINT custom_indicator_evaluation_payload_fk, \
                     DROP COLUMN execution_step_id, \
                     DROP COLUMN payload_pointers, \
                     DROP COLUMN payload_id; \
                 DROP TABLE task_execution_payload_chunk; \
                 DROP TRIGGER task_execution_payload_immutable ON task_execution_payload; \
                 DROP FUNCTION protect_execution_payload(); \
                 DROP TABLE task_execution_payload; \
                 DROP TRIGGER task_execution_definition_snapshot_immutable ON task_execution_definition_snapshot; \
                 DROP FUNCTION reject_execution_definition_snapshot_change(); \
                 DROP TABLE task_execution_definition_snapshot; \
                 DROP FUNCTION protect_execution_payload_chunk();",
            )
            .await?;
        Ok(())
    }
}
