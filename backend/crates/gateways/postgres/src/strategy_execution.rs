use async_trait::async_trait;
use core_application::{
    strategy_execution::{
        DefinitionSnapshot, PayloadCaptureMetadata, PayloadChunk, PayloadManifest, PayloadSeal,
        PayloadStatus, StrategyExecutionRepository, StrategyExecutionRepositoryError,
    },
    unit_of_work::UnitOfWorkTransaction,
};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{Set, Unchanged},
    ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, FromQueryResult, IntoActiveModel,
    QueryFilter, QueryOrder, QuerySelect, Statement,
    sea_query::{LockType, OnConflict},
};
use uuid::Uuid;

use crate::{
    DatabaseHandle,
    entities::{
        task_execution_definition_snapshot, task_execution_payload, task_execution_payload_chunk,
    },
    persistence::persistence_error,
    transaction::transaction_ref,
};

#[derive(Clone)]
pub struct PostgresStrategyExecutionRepository {
    db: DatabaseHandle,
}

impl PostgresStrategyExecutionRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl StrategyExecutionRepository for PostgresStrategyExecutionRepository {
    async fn find_definition_snapshot(
        &self,
        task_id: Uuid,
        definition_key: &str,
    ) -> Result<Option<DefinitionSnapshot>, StrategyExecutionRepositoryError> {
        task_execution_definition_snapshot::Entity::find_by_id((
            task_id,
            definition_key.to_string(),
        ))
        .one(&self.db)
        .await
        .map(|row| row.map(Into::into))
        .map_err(database_error)
    }

    async fn list_definition_snapshots(
        &self,
        task_id: Uuid,
    ) -> Result<Vec<DefinitionSnapshot>, StrategyExecutionRepositoryError> {
        let rows = task_execution_definition_snapshot::Entity::find()
            .filter(task_execution_definition_snapshot::Column::TaskId.eq(task_id))
            .order_by_asc(task_execution_definition_snapshot::Column::DefinitionKey)
            .all(&self.db)
            .await
            .map_err(database_error)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn insert_definition_snapshot(
        &self,
        transaction: &UnitOfWorkTransaction,
        snapshot: DefinitionSnapshot,
    ) -> Result<DefinitionSnapshot, StrategyExecutionRepositoryError> {
        let transaction = transaction_ref(transaction)
            .ok_or(StrategyExecutionRepositoryError::InvalidTransaction)?;
        task_execution_definition_snapshot::Entity::insert(
            task_execution_definition_snapshot::ActiveModel {
                task_id: Set(snapshot.task_id),
                definition_key: Set(snapshot.definition_key.clone()),
                definition: Set(snapshot.definition.clone()),
                digest: Set(snapshot.digest),
                created_at: Set(snapshot.created_at),
            },
        )
        .on_conflict(
            OnConflict::columns([
                task_execution_definition_snapshot::Column::TaskId,
                task_execution_definition_snapshot::Column::DefinitionKey,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(transaction)
        .await
        .map_err(database_error)?;

        task_execution_definition_snapshot::Entity::find_by_id((
            snapshot.task_id,
            snapshot.definition_key,
        ))
        .one(transaction)
        .await
        .map_err(database_error)?
        .map(Into::into)
        .ok_or(StrategyExecutionRepositoryError::InconsistentPayload)
    }

    async fn insert_payload(
        &self,
        transaction: &UnitOfWorkTransaction,
        payload: PayloadManifest,
    ) -> Result<(), StrategyExecutionRepositoryError> {
        let transaction = transaction_ref(transaction)
            .ok_or(StrategyExecutionRepositoryError::InvalidTransaction)?;
        task_execution_payload::ActiveModel {
            id: Set(payload.id),
            task_id: Set(payload.task_id),
            execution_step_id: Set(payload.execution_step_id),
            kind: Set(payload.kind.as_str().to_string()),
            source: Set(payload.source),
            source_revision: Set(payload.source_revision),
            source_locator: Set(payload.source_locator),
            source_metadata: Set(payload.source_metadata),
            schema: Set(payload.schema),
            capture_metadata: Set(payload
                .capture_metadata
                .map(serde_json::to_value)
                .transpose()
                .map_err(|_| StrategyExecutionRepositoryError::InconsistentPayload)?),
            root_is_array: Set(payload.root_is_array),
            chunk_count: Set(0),
            item_count: Set(0),
            byte_count: Set(0),
            digest: Set(None),
            status: Set(PayloadStatus::Staging.as_str().to_string()),
            incomplete_reason: Set(None),
            created_at: Set(payload.created_at),
            sealed_at: Set(None),
        }
        .insert(transaction)
        .await
        .map(|_| ())
        .map_err(database_error)
    }

    async fn find_payload_for_update(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        payload_id: Uuid,
    ) -> Result<Option<PayloadManifest>, StrategyExecutionRepositoryError> {
        let transaction = transaction_ref(transaction)
            .ok_or(StrategyExecutionRepositoryError::InvalidTransaction)?;
        let row = task_execution_payload::Entity::find_by_id(payload_id)
            .filter(task_execution_payload::Column::TaskId.eq(task_id))
            .lock(LockType::Update)
            .one(transaction)
            .await
            .map_err(database_error)?;
        row.map(TryInto::try_into).transpose()
    }

    async fn list_payload_chunks_for_update(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        payload_id: Uuid,
    ) -> Result<Vec<PayloadChunk>, StrategyExecutionRepositoryError> {
        let transaction = transaction_ref(transaction)
            .ok_or(StrategyExecutionRepositoryError::InvalidTransaction)?;
        if !payload_exists(transaction, task_id, payload_id).await? {
            return Err(StrategyExecutionRepositoryError::InconsistentPayload);
        }
        let rows = task_execution_payload_chunk::Entity::find()
            .filter(task_execution_payload_chunk::Column::PayloadId.eq(payload_id))
            .order_by_asc(task_execution_payload_chunk::Column::Sequence)
            .lock(LockType::Update)
            .all(transaction)
            .await
            .map_err(database_error)?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn append_payload_chunk(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        chunk: PayloadChunk,
    ) -> Result<PayloadManifest, StrategyExecutionRepositoryError> {
        let transaction = transaction_ref(transaction)
            .ok_or(StrategyExecutionRepositoryError::InvalidTransaction)?;
        let manifest = task_execution_payload::Entity::find_by_id(chunk.payload_id)
            .filter(task_execution_payload::Column::TaskId.eq(task_id))
            .lock(LockType::Update)
            .one(transaction)
            .await
            .map_err(database_error)?
            .ok_or(StrategyExecutionRepositoryError::InconsistentPayload)?;
        if manifest.status != PayloadStatus::Staging.as_str()
            || manifest.chunk_count != i64::try_from(chunk.sequence).unwrap_or(i64::MIN)
        {
            return Err(StrategyExecutionRepositoryError::InvalidPayloadState);
        }
        let chunk_count = manifest
            .chunk_count
            .checked_add(1)
            .ok_or(StrategyExecutionRepositoryError::PayloadTooLarge)?;
        let item_count = manifest
            .item_count
            .checked_add(
                i64::try_from(chunk.item_count)
                    .map_err(|_| StrategyExecutionRepositoryError::PayloadTooLarge)?,
            )
            .ok_or(StrategyExecutionRepositoryError::PayloadTooLarge)?;
        let byte_count = manifest
            .byte_count
            .checked_add(
                i64::try_from(chunk.byte_count)
                    .map_err(|_| StrategyExecutionRepositoryError::PayloadTooLarge)?,
            )
            .ok_or(StrategyExecutionRepositoryError::PayloadTooLarge)?;

        task_execution_payload_chunk::ActiveModel {
            payload_id: Set(chunk.payload_id),
            sequence: Set(i64::try_from(chunk.sequence)
                .map_err(|_| StrategyExecutionRepositoryError::PayloadTooLarge)?),
            start_item: Set(i64::try_from(chunk.start_item)
                .map_err(|_| StrategyExecutionRepositoryError::PayloadTooLarge)?),
            data: Set(chunk.data),
            byte_count: Set(i64::try_from(chunk.byte_count)
                .map_err(|_| StrategyExecutionRepositoryError::PayloadTooLarge)?),
            item_count: Set(i64::try_from(chunk.item_count)
                .map_err(|_| StrategyExecutionRepositoryError::PayloadTooLarge)?),
        }
        .insert(transaction)
        .await
        .map_err(database_error)?;

        let updated = task_execution_payload::ActiveModel {
            id: Unchanged(manifest.id),
            chunk_count: Set(chunk_count),
            item_count: Set(item_count),
            byte_count: Set(byte_count),
            ..Default::default()
        }
        .update(transaction)
        .await
        .map_err(database_error)?;
        updated.try_into()
    }

    async fn update_payload_capture_metadata(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        payload_id: Uuid,
        metadata: PayloadCaptureMetadata,
    ) -> Result<(), StrategyExecutionRepositoryError> {
        let transaction = transaction_ref(transaction)
            .ok_or(StrategyExecutionRepositoryError::InvalidTransaction)?;
        let mut active = task_execution_payload::Entity::find_by_id(payload_id)
            .filter(task_execution_payload::Column::TaskId.eq(task_id))
            .lock(LockType::Update)
            .one(transaction)
            .await
            .map_err(database_error)?
            .ok_or(StrategyExecutionRepositoryError::InconsistentPayload)?
            .into_active_model();
        if active.status.as_ref() != PayloadStatus::Staging.as_str() {
            return Err(StrategyExecutionRepositoryError::InvalidPayloadState);
        }
        active.capture_metadata =
            Set(Some(serde_json::to_value(metadata).map_err(|_| {
                StrategyExecutionRepositoryError::InconsistentPayload
            })?));
        active
            .update(transaction)
            .await
            .map(|_| ())
            .map_err(database_error)
    }

    async fn mark_payload_sealed(
        &self,
        transaction: &UnitOfWorkTransaction,
        seal: PayloadSeal,
    ) -> Result<PayloadManifest, StrategyExecutionRepositoryError> {
        let transaction = transaction_ref(transaction)
            .ok_or(StrategyExecutionRepositoryError::InvalidTransaction)?;
        let mut active = task_execution_payload::Entity::find_by_id(seal.payload_id)
            .filter(task_execution_payload::Column::TaskId.eq(seal.task_id))
            .one(transaction)
            .await
            .map_err(database_error)?
            .ok_or(StrategyExecutionRepositoryError::InconsistentPayload)?
            .into_active_model();
        if active.status.as_ref() != PayloadStatus::Staging.as_str() {
            return Err(StrategyExecutionRepositoryError::InvalidPayloadState);
        }
        active.digest = Set(Some(seal.digest));
        active.item_count = Set(i64::try_from(seal.item_count)
            .map_err(|_| StrategyExecutionRepositoryError::PayloadTooLarge)?);
        active.byte_count = Set(i64::try_from(seal.byte_count)
            .map_err(|_| StrategyExecutionRepositoryError::PayloadTooLarge)?);
        active.status = Set(PayloadStatus::Sealed.as_str().to_string());
        active.sealed_at = Set(Some(seal.sealed_at));
        let updated = active.update(transaction).await.map_err(database_error)?;
        updated.try_into()
    }

    async fn mark_payload_incomplete(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        payload_id: Uuid,
        reason: String,
    ) -> Result<(), StrategyExecutionRepositoryError> {
        let transaction = transaction_ref(transaction)
            .ok_or(StrategyExecutionRepositoryError::InvalidTransaction)?;
        let mut active = task_execution_payload::Entity::find_by_id(payload_id)
            .filter(task_execution_payload::Column::TaskId.eq(task_id))
            .one(transaction)
            .await
            .map_err(database_error)?
            .ok_or(StrategyExecutionRepositoryError::InconsistentPayload)?
            .into_active_model();
        if active.status.as_ref() != PayloadStatus::Staging.as_str() {
            return Err(StrategyExecutionRepositoryError::InvalidPayloadState);
        }
        active.status = Set(PayloadStatus::Incomplete.as_str().to_string());
        active.incomplete_reason = Set(Some(reason));
        active
            .update(transaction)
            .await
            .map(|_| ())
            .map_err(database_error)
    }

    async fn find_payload(
        &self,
        task_id: Uuid,
        payload_id: Uuid,
    ) -> Result<Option<PayloadManifest>, StrategyExecutionRepositoryError> {
        let row = task_execution_payload::Entity::find_by_id(payload_id)
            .filter(task_execution_payload::Column::TaskId.eq(task_id))
            .one(&self.db)
            .await
            .map_err(database_error)?;
        row.map(TryInto::try_into).transpose()
    }

    async fn list_payload_chunks(
        &self,
        task_id: Uuid,
        payload_id: Uuid,
    ) -> Result<Vec<PayloadChunk>, StrategyExecutionRepositoryError> {
        if !payload_exists(&self.db, task_id, payload_id).await? {
            return Err(StrategyExecutionRepositoryError::InconsistentPayload);
        }
        let rows = task_execution_payload_chunk::Entity::find()
            .filter(task_execution_payload_chunk::Column::PayloadId.eq(payload_id))
            .order_by_asc(task_execution_payload_chunk::Column::Sequence)
            .all(&self.db)
            .await
            .map_err(database_error)?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn list_payload_chunks_for_range(
        &self,
        task_id: Uuid,
        payload_id: Uuid,
        start_item: u64,
        item_count: usize,
    ) -> Result<Vec<PayloadChunk>, StrategyExecutionRepositoryError> {
        if !payload_exists(&self.db, task_id, payload_id).await? {
            return Err(StrategyExecutionRepositoryError::InconsistentPayload);
        }
        if item_count == 0 {
            return Ok(Vec::new());
        }
        let start = i64::try_from(start_item)
            .map_err(|_| StrategyExecutionRepositoryError::PayloadTooLarge)?;
        let count = i64::try_from(item_count)
            .map_err(|_| StrategyExecutionRepositoryError::PayloadTooLarge)?;
        let end = start
            .checked_add(count)
            .ok_or(StrategyExecutionRepositoryError::PayloadTooLarge)?;
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT c.payload_id, c.sequence, c.start_item, c.data, c.byte_count, c.item_count \
                 FROM task_execution_payload_chunk AS c \
                 INNER JOIN task_execution_payload AS p ON p.id = c.payload_id \
                 WHERE p.task_id = $1 AND c.payload_id = $2 \
                   AND c.start_item < $3 AND c.start_item + c.item_count > $4 \
                 ORDER BY c.sequence",
                [task_id.into(), payload_id.into(), end.into(), start.into()],
            ))
            .await
            .map_err(database_error)?;
        rows.iter()
            .map(|row| {
                task_execution_payload_chunk::Model::from_query_result(row, "")
                    .map_err(database_error)?
                    .try_into()
            })
            .collect()
    }
}

async fn payload_exists<C: sea_orm::ConnectionTrait>(
    connection: &C,
    task_id: Uuid,
    payload_id: Uuid,
) -> Result<bool, StrategyExecutionRepositoryError> {
    task_execution_payload::Entity::find_by_id(payload_id)
        .filter(task_execution_payload::Column::TaskId.eq(task_id))
        .one(connection)
        .await
        .map(|row| row.is_some())
        .map_err(database_error)
}

fn database_error(error: sea_orm::DbErr) -> StrategyExecutionRepositoryError {
    StrategyExecutionRepositoryError::Database(persistence_error(error))
}

impl From<task_execution_definition_snapshot::Model> for DefinitionSnapshot {
    fn from(model: task_execution_definition_snapshot::Model) -> Self {
        Self {
            task_id: model.task_id,
            definition_key: model.definition_key,
            definition: model.definition,
            digest: model.digest,
            created_at: model.created_at,
        }
    }
}
