use gateway_postgres::DatabaseHandle;
use gateway_postgres::entities::annotation;
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

pub async fn set_test_annotation_execution_step_id(
    db: &DatabaseHandle,
    annotation_id: Uuid,
    execution_step_id: Uuid,
) {
    annotation::Entity::update_many()
        .col_expr(
            annotation::Column::ExecutionStepId,
            Expr::value(Some(execution_step_id)),
        )
        .filter(annotation::Column::Id.eq(annotation_id))
        .exec(db)
        .await
        .expect("set test annotation execution step ID");
}
