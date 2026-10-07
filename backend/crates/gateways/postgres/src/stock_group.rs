use async_trait::async_trait;
use core_application::stock_group::{
    GroupAxis, NewStockGroup, StockGroup, StockGroupMembership, StockGroupRepository,
    StockGroupRepositoryError,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{NotSet, Set, Unchanged};
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, JoinType, QueryFilter, QueryOrder, QuerySelect,
    RelationTrait,
};
use uuid::Uuid;

use crate::entities::{group_axis, stock, stock_group, stock_group_member};
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref;

#[derive(Clone, Default)]
pub struct PostgresStockGroupRepository;

impl PostgresStockGroupRepository {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StockGroupRepository for PostgresStockGroupRepository {
    async fn find_axis_by_key(
        &self,
        unit_of_work: &UnitOfWorkTransaction,
        axis_key: &str,
    ) -> Result<Option<GroupAxis>, StockGroupRepositoryError> {
        let transaction =
            transaction_ref(unit_of_work).ok_or(StockGroupRepositoryError::InvalidTransaction)?;
        group_axis::Entity::find()
            .filter(group_axis::Column::Key.eq(axis_key))
            .one(transaction)
            .await
            .map(|row| {
                row.map(|row| GroupAxis {
                    id: row.id,
                    key: row.key,
                    derive_from: row.derive_from,
                })
            })
            .map_err(repository_error)
    }

    async fn find_group(
        &self,
        unit_of_work: &UnitOfWorkTransaction,
        axis_id: Uuid,
        axis_key: &str,
        group_key: &str,
    ) -> Result<Option<StockGroup>, StockGroupRepositoryError> {
        let transaction =
            transaction_ref(unit_of_work).ok_or(StockGroupRepositoryError::InvalidTransaction)?;
        stock_group::Entity::find()
            .filter(stock_group::Column::AxisId.eq(axis_id))
            .filter(stock_group::Column::Key.eq(group_key))
            .one(transaction)
            .await
            .map(|row| row.map(|row| to_domain(row, axis_key)))
            .map_err(repository_error)
    }

    async fn find_codes_by_derive_from(
        &self,
        unit_of_work: &UnitOfWorkTransaction,
        derive_from: &str,
        group_key: &str,
    ) -> Result<Vec<Option<String>>, StockGroupRepositoryError> {
        let transaction =
            transaction_ref(unit_of_work).ok_or(StockGroupRepositoryError::InvalidTransaction)?;
        stock_group::Entity::find()
            .join(JoinType::InnerJoin, stock_group::Relation::GroupAxis.def())
            .select_only()
            .column(stock_group::Column::Code)
            .filter(group_axis::Column::DeriveFrom.eq(derive_from))
            .filter(stock_group::Column::Key.eq(group_key))
            .into_tuple::<Option<String>>()
            .all(transaction)
            .await
            .map_err(repository_error)
    }

    async fn insert_group(
        &self,
        unit_of_work: &UnitOfWorkTransaction,
        group: NewStockGroup,
    ) -> Result<StockGroup, StockGroupRepositoryError> {
        let transaction =
            transaction_ref(unit_of_work).ok_or(StockGroupRepositoryError::InvalidTransaction)?;
        stock_group::Entity::insert(stock_group::ActiveModel {
            id: Set(group.id),
            axis_id: Set(group.axis_id),
            key: Set(group.key),
            name: Set(group.name),
            description: Set(group.description),
            code: Set(None),
        })
        .exec_with_returning(transaction)
        .await
        .map(|row| to_domain(row, &group.axis_key))
        .map_err(repository_error)
    }

    async fn update_group(
        &self,
        unit_of_work: &UnitOfWorkTransaction,
        group: StockGroup,
    ) -> Result<StockGroup, StockGroupRepositoryError> {
        let transaction =
            transaction_ref(unit_of_work).ok_or(StockGroupRepositoryError::InvalidTransaction)?;
        stock_group::ActiveModel {
            id: Unchanged(group.id),
            axis_id: Unchanged(group.axis_id),
            key: Unchanged(group.key),
            name: Set(group.name),
            description: Set(group.description),
            code: NotSet,
        }
        .update(transaction)
        .await
        .map(|row| to_domain(row, &group.axis_key))
        .map_err(repository_error)
    }

    async fn stock_exists(
        &self,
        unit_of_work: &UnitOfWorkTransaction,
        stock_id: &str,
    ) -> Result<bool, StockGroupRepositoryError> {
        let transaction =
            transaction_ref(unit_of_work).ok_or(StockGroupRepositoryError::InvalidTransaction)?;
        stock::Entity::find_by_id(stock_id.to_owned())
            .one(transaction)
            .await
            .map(|row| row.is_some())
            .map_err(repository_error)
    }

    async fn list_stock_ids(
        &self,
        unit_of_work: &UnitOfWorkTransaction,
        group_id: Uuid,
    ) -> Result<Vec<String>, StockGroupRepositoryError> {
        let transaction =
            transaction_ref(unit_of_work).ok_or(StockGroupRepositoryError::InvalidTransaction)?;
        stock_group_member::Entity::find()
            .filter(stock_group_member::Column::GroupId.eq(group_id))
            .order_by_asc(stock_group_member::Column::StockId)
            .all(transaction)
            .await
            .map(|rows| rows.into_iter().map(|row| row.stock_id).collect())
            .map_err(repository_error)
    }

    async fn list_memberships(
        &self,
        unit_of_work: &UnitOfWorkTransaction,
        stock_ids: &[String],
        axis_keys: Option<&[String]>,
    ) -> Result<Vec<StockGroupMembership>, StockGroupRepositoryError> {
        if stock_ids.is_empty() || axis_keys.is_some_and(|axis_keys| axis_keys.is_empty()) {
            return Ok(Vec::new());
        }
        let transaction =
            transaction_ref(unit_of_work).ok_or(StockGroupRepositoryError::InvalidTransaction)?;
        let query = stock_group_member::Entity::find()
            .join(
                JoinType::InnerJoin,
                stock_group_member::Relation::StockGroup.def(),
            )
            .join(JoinType::InnerJoin, stock_group::Relation::GroupAxis.def())
            .select_only()
            .column_as(group_axis::Column::Key, "axis_key")
            .column_as(stock_group::Column::Key, "group_key")
            .column(stock_group_member::Column::StockId)
            .filter(stock_group_member::Column::StockId.is_in(stock_ids.to_vec()))
            .order_by_asc(group_axis::Column::Key)
            .order_by_asc(stock_group::Column::Key)
            .order_by_asc(stock_group_member::Column::StockId);
        let query = if let Some(axis_keys) = axis_keys {
            query.filter(group_axis::Column::Key.is_in(axis_keys.to_vec()))
        } else {
            query
        };
        query
            .into_model::<StockGroupMembershipRow>()
            .all(transaction)
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|row| StockGroupMembership {
                        axis_key: row.axis_key,
                        group_key: row.group_key,
                        stock_id: row.stock_id,
                    })
                    .collect()
            })
            .map_err(repository_error)
    }

    async fn add_stock(
        &self,
        unit_of_work: &UnitOfWorkTransaction,
        group_id: Uuid,
        stock_id: &str,
    ) -> Result<bool, StockGroupRepositoryError> {
        let transaction =
            transaction_ref(unit_of_work).ok_or(StockGroupRepositoryError::InvalidTransaction)?;
        let rows_affected = stock_group_member::Entity::insert(stock_group_member::ActiveModel {
            stock_id: Set(stock_id.to_owned()),
            group_id: Set(group_id),
            created_at: NotSet,
        })
        .on_conflict(
            OnConflict::columns([
                stock_group_member::Column::StockId,
                stock_group_member::Column::GroupId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(transaction)
        .await
        .map_err(repository_error)?;
        Ok(rows_affected > 0)
    }

    async fn remove_stock(
        &self,
        unit_of_work: &UnitOfWorkTransaction,
        group_id: Uuid,
        stock_id: &str,
    ) -> Result<bool, StockGroupRepositoryError> {
        let transaction =
            transaction_ref(unit_of_work).ok_or(StockGroupRepositoryError::InvalidTransaction)?;
        stock_group_member::Entity::delete_many()
            .filter(stock_group_member::Column::GroupId.eq(group_id))
            .filter(stock_group_member::Column::StockId.eq(stock_id))
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }
}

#[derive(Debug, sea_orm::FromQueryResult)]
struct StockGroupMembershipRow {
    axis_key: String,
    group_key: String,
    stock_id: String,
}

fn repository_error(error: sea_orm::DbErr) -> StockGroupRepositoryError {
    StockGroupRepositoryError::Database(persistence_error(error))
}

fn to_domain(row: stock_group::Model, axis_key: &str) -> StockGroup {
    StockGroup {
        id: row.id,
        axis_id: row.axis_id,
        axis_key: axis_key.to_owned(),
        key: row.key,
        name: row.name,
        description: row.description,
    }
}
