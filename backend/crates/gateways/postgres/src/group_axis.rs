use async_trait::async_trait;
use core_application::group_axis::{
    GroupAxis, GroupAxisRepository, GroupAxisRepositoryError, NewGroupAxis,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{Set, Unchanged};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

use crate::DatabaseHandle;
use crate::entities::group_axis;
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref;

#[derive(Clone)]
pub struct PostgresGroupAxisRepository {
    db: DatabaseHandle,
}

impl PostgresGroupAxisRepository {
    pub fn new(db: impl Into<DatabaseHandle>) -> Self {
        Self { db: db.into() }
    }
}

#[async_trait]
impl GroupAxisRepository for PostgresGroupAxisRepository {
    async fn list(&self) -> Result<Vec<GroupAxis>, GroupAxisRepositoryError> {
        group_axis::Entity::find()
            .order_by_asc(group_axis::Column::Key)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_group_axis).collect())
            .map_err(repository_error)
    }

    async fn find_by_key(
        &self,
        transaction: &UnitOfWorkTransaction,
        key: &str,
    ) -> Result<Option<GroupAxis>, GroupAxisRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(GroupAxisRepositoryError::InvalidTransaction)?;
        group_axis::Entity::find()
            .filter(group_axis::Column::Key.eq(key))
            .one(transaction)
            .await
            .map(|row| row.map(to_group_axis))
            .map_err(repository_error)
    }

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        axis: NewGroupAxis,
    ) -> Result<GroupAxis, GroupAxisRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(GroupAxisRepositoryError::InvalidTransaction)?;
        group_axis::Entity::insert(group_axis::ActiveModel {
            key: Set(axis.key),
            name: Set(axis.name),
            description: Set(axis.description),
            sync_source: Set(axis.sync_source),
            ..Default::default()
        })
        .exec_with_returning(transaction)
        .await
        .map(to_group_axis)
        .map_err(repository_error)
    }

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        axis: GroupAxis,
    ) -> Result<GroupAxis, GroupAxisRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(GroupAxisRepositoryError::InvalidTransaction)?;
        group_axis::ActiveModel {
            id: Unchanged(axis.id),
            key: Unchanged(axis.key),
            name: Set(axis.name),
            description: Set(axis.description),
            sync_source: Set(axis.sync_source),
        }
        .update(transaction)
        .await
        .map(to_group_axis)
        .map_err(repository_error)
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        key: &str,
    ) -> Result<bool, GroupAxisRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(GroupAxisRepositoryError::InvalidTransaction)?;
        group_axis::Entity::delete_many()
            .filter(group_axis::Column::Key.eq(key))
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }
}

fn repository_error(error: sea_orm::DbErr) -> GroupAxisRepositoryError {
    GroupAxisRepositoryError::Database(persistence_error(error))
}

fn to_group_axis(model: group_axis::Model) -> GroupAxis {
    GroupAxis {
        id: model.id,
        key: model.key,
        name: model.name,
        description: model.description,
        sync_source: model.sync_source,
    }
}
