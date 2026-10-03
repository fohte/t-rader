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

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use core_application::annotation::{AnnotationListQuery, AnnotationReadQuery};
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::EntityTrait;
    use uuid::Uuid;

    use super::PostgresAnnotationReadQuery;
    use crate::DatabaseHandle;
    use crate::entities::{annotation, strategy};

    #[backend_test_macros::database_test]
    async fn list_without_strategy_filter_includes_other_strategies_and_unscoped_annotations(
        db: DatabaseHandle,
    ) {
        let strategy_a = insert_strategy(&db, "strategy-a").await;
        let strategy_b = insert_strategy(&db, "strategy-b").await;
        let strategy_annotation_id = insert_annotation(&db, Some(strategy_a)).await;
        let other_strategy_annotation_id = insert_annotation(&db, Some(strategy_b)).await;
        let unscoped_annotation_id = insert_annotation(&db, None).await;
        let query = PostgresAnnotationReadQuery::new(db);

        let annotations = query
            .list(AnnotationListQuery {
                strategy_id: None,
                ..Default::default()
            })
            .await
            .expect("list annotations without a strategy filter");
        let mut actual = annotations
            .into_iter()
            .map(|annotation| (annotation.id, annotation.strategy_id))
            .collect::<Vec<_>>();
        actual.sort_by_key(|(id, _)| *id);

        let mut expected = vec![
            (strategy_annotation_id, Some(strategy_a)),
            (other_strategy_annotation_id, Some(strategy_b)),
            (unscoped_annotation_id, None),
        ];
        expected.sort_by_key(|(id, _)| *id);

        assert_eq!(actual, expected);
    }

    async fn insert_strategy(db: &DatabaseHandle, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        strategy::ActiveModel {
            id: Set(id),
            name: Set(name.to_string()),
            description: Set(None),
            sort_order: Set(0),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert test strategy");
        id
    }

    async fn insert_annotation(db: &DatabaseHandle, strategy_id: Option<Uuid>) -> Uuid {
        let id = Uuid::new_v4();
        annotation::Entity::insert(annotation::ActiveModel {
            id: Set(id),
            strategy_id: Set(strategy_id),
            target_symbol: Set("sample-symbol".to_string()),
            target_kind: Set("sample-kind".to_string()),
            timestamp: Set(Utc::now().fixed_offset()),
            price: Set(None),
            text: Set("sample annotation".to_string()),
            status: Set("approved".to_string()),
            linked_note_id: Set(None),
            created_by_kind: Set("human".to_string()),
            created_at: NotSet,
            updated_at: NotSet,
            execution_step_id: Set(None),
            execution_task_id: Set(None),
        })
        .exec_without_returning(db)
        .await
        .expect("insert test annotation");
        id
    }
}
