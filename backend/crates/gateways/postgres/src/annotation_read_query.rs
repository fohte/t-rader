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
        limit: u64,
    ) -> Result<Vec<RecentAnnotation>, AnnotationReadQueryError> {
        annotation::Entity::find()
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
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::EntityTrait;
    use uuid::Uuid;

    use super::PostgresAnnotationReadQuery;
    use crate::DatabaseHandle;
    use crate::entities::annotation;

    #[backend_test_macros::database_test]
    async fn list_includes_all_annotations(db: DatabaseHandle) {
        let first_annotation_id = insert_annotation(&db).await;
        let second_annotation_id = insert_annotation(&db).await;
        let third_annotation_id = insert_annotation(&db).await;
        let query = PostgresAnnotationReadQuery::new(db);

        let annotations = query
            .list(AnnotationListQuery::default())
            .await
            .expect("list annotations globally");
        let mut actual = annotations
            .into_iter()
            .map(|annotation| annotation.id)
            .collect::<Vec<_>>();
        actual.sort();

        let mut expected = vec![
            first_annotation_id,
            second_annotation_id,
            third_annotation_id,
        ];
        expected.sort();

        assert_eq!(actual, expected);
    }

    async fn insert_annotation(db: &DatabaseHandle) -> Uuid {
        let id = Uuid::new_v4();
        annotation::Entity::insert(annotation::ActiveModel {
            id: Set(id),
            target_symbol: Set("sample-symbol".to_string()),
            target_kind: Set("sample-kind".to_string()),
            timestamp: Set(Utc::now().fixed_offset()),
            timestamp_start: Set(None),
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
