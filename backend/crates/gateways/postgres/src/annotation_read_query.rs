use async_trait::async_trait;
use core_application::annotation::{
    Annotation, AnnotationListQuery, AnnotationReadQuery, AnnotationReadQueryError,
    RecentAnnotation,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::annotation::to_domain;
use crate::entities::annotation;
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresAnnotationReadQuery {
    db: DatabaseHandle,
}

impl PostgresAnnotationReadQuery {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AnnotationReadQuery for PostgresAnnotationReadQuery {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Annotation>, AnnotationReadQueryError> {
        annotation::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map(|row| row.map(to_domain))
            .map_err(query_error)
    }

    async fn list(
        &self,
        query: AnnotationListQuery,
    ) -> Result<Vec<Annotation>, AnnotationReadQueryError> {
        let mut select = annotation::Entity::find();
        if let Some(strategy_id) = query.strategy_id {
            select = select.filter(annotation::Column::StrategyId.eq(strategy_id));
        }
        if let Some(target_symbol) = query.target_symbol {
            select = select.filter(annotation::Column::TargetSymbol.eq(target_symbol));
        }
        select = select.order_by_desc(annotation::Column::Timestamp);
        if let Some(limit) = query.limit {
            select = select.limit(limit);
        }
        select
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_domain).collect())
            .map_err(query_error)
    }

    async fn list_recent(
        &self,
        strategy_id: Uuid,
        limit: u64,
    ) -> Result<Vec<RecentAnnotation>, AnnotationReadQueryError> {
        annotation::Entity::find()
            .filter(annotation::Column::StrategyId.eq(strategy_id))
            .order_by_desc(annotation::Column::UpdatedAt)
            .limit(limit)
            .all(&self.db)
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|row| RecentAnnotation {
                        id: row.id,
                        target_symbol: row.target_symbol,
                        target_kind: row.target_kind,
                        status: row.status,
                        created_by_kind: row.created_by_kind,
                        updated_at: row.updated_at,
                    })
                    .collect()
            })
            .map_err(query_error)
    }
}

fn query_error(error: sea_orm::DbErr) -> AnnotationReadQueryError {
    AnnotationReadQueryError::Database(persistence_error(error))
}
