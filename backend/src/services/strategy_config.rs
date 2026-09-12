//! 戦略 1 行の設定 (name / description / sort_order) の
//! 取得・作成・部分更新・削除の共通経路。
//!
//! REST (`handlers::strategies`) と管理 MCP (`mcp::mgmt`) の両方から同じカラムへの書き込みが
//! 発生しうるため、検証・DB 更新・change_history 記録をここに集約し、書き込み経路を 1 つに
//! 保つ。

use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter,
    TransactionTrait,
};
use serde_json::json;
use uuid::Uuid;

use crate::entities::strategy;
use crate::error::AppError;
use crate::services::change_history::{self, Actor, Op, TargetKind};

pub async fn find_or_404(db: &DatabaseConnection, id: Uuid) -> Result<strategy::Model, AppError> {
    strategy::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("strategy {id} not found")))
}

pub fn validate_name(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(AppError::Validation("name must not be empty".into()));
    }
    Ok(trimmed)
}

pub struct CreateStrategy {
    pub name: String,
    pub description: Option<String>,
    pub sort_order: i32,
}

pub async fn create(
    db: &DatabaseConnection,
    actor: Actor,
    params: CreateStrategy,
) -> Result<strategy::Model, AppError> {
    let name = validate_name(&params.name)?;
    let id = Uuid::new_v4();

    let model = strategy::ActiveModel {
        id: Set(id),
        name: Set(name.clone()),
        description: Set(params.description),
        sort_order: Set(params.sort_order),
        created_at: NotSet,
        updated_at: NotSet,
    };
    let txn = db.begin().await?;
    let created = strategy::Entity::insert(model)
        .exec_with_returning(&txn)
        .await?;
    change_history::record_as(
        &txn,
        actor,
        TargetKind::Strategy,
        id,
        Op::Create,
        json!({
            "name": created.name,
            "sort_order": created.sort_order,
            "description": created.description,
        }),
        Some(format!("created strategy {name}")),
    )
    .await?;
    txn.commit().await?;

    Ok(created)
}

#[derive(Default)]
pub struct StrategyUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub sort_order: Option<i32>,
}

pub async fn update(
    db: &DatabaseConnection,
    actor: Actor,
    id: Uuid,
    payload: StrategyUpdate,
) -> Result<strategy::Model, AppError> {
    let current = find_or_404(db, id).await?;
    let mut active = current.clone().into_active_model();
    let mut diff = serde_json::Map::new();

    if let Some(name) = payload.name {
        let name = validate_name(&name)?;
        diff.insert("name".into(), json!({ "from": current.name, "to": name }));
        active.name = Set(name);
    }
    if let Some(description) = payload.description {
        diff.insert(
            "description".into(),
            json!({ "from": current.description, "to": description }),
        );
        active.description = Set(Some(description));
    }
    if let Some(sort_order) = payload.sort_order {
        diff.insert(
            "sort_order".into(),
            json!({ "from": current.sort_order, "to": sort_order }),
        );
        active.sort_order = Set(sort_order);
    }
    active.updated_at = Set(chrono::Utc::now().fixed_offset());

    let txn = db.begin().await?;
    let updated = active.update(&txn).await?;
    if !diff.is_empty() {
        change_history::record_as(
            &txn,
            actor,
            TargetKind::Strategy,
            id,
            Op::Update,
            serde_json::Value::Object(diff),
            None,
        )
        .await?;
    }
    txn.commit().await?;

    Ok(updated)
}

/// 戦略を削除する。関連リソース (note / annotation / trade / trigger 等) は DB の
/// `on_delete = Cascade` で連鎖削除される。
pub async fn delete(db: &DatabaseConnection, actor: Actor, id: Uuid) -> Result<(), AppError> {
    let txn = db.begin().await?;
    let result = strategy::Entity::delete_by_id(id).exec(&txn).await?;
    if result.rows_affected == 0 {
        return Err(AppError::NotFound(format!("strategy {id} not found")));
    }
    change_history::record_as(
        &txn,
        actor,
        TargetKind::Strategy,
        id,
        Op::Delete,
        json!({}),
        None,
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

/// `expected_name` が現在の戦略名と一致する場合のみ削除する。confirm_name チェックと
/// 削除を単一クエリの条件にまとめることで、事前チェックと削除実行の間で名前が変わる
/// race を閉じる (呼び出し元が事前に `find_or_404` で確認していても、その後の再確認は
/// このクエリ自体が兼ねる)。
pub async fn delete_confirmed(
    db: &DatabaseConnection,
    actor: Actor,
    id: Uuid,
    expected_name: &str,
) -> Result<(), AppError> {
    let txn = db.begin().await?;
    let result = strategy::Entity::delete_many()
        .filter(strategy::Column::Id.eq(id))
        .filter(strategy::Column::Name.eq(expected_name))
        .exec(&txn)
        .await?;
    if result.rows_affected == 0 {
        return Err(AppError::NotFound(format!(
            "strategy {id} not found or name changed since confirmation"
        )));
    }
    change_history::record_as(
        &txn,
        actor,
        TargetKind::Strategy,
        id,
        Op::Delete,
        json!({}),
        None,
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::*;
    use crate::testing::{create_test_db, insert_test_strategy};

    #[sqlx::test(migrations = false)]
    async fn delete_records_change_history_with_given_actor(pool: PgPool) {
        let db = create_test_db(pool).await;
        let id = insert_test_strategy(&db, "s").await;

        delete(&db, Actor::Llm { label: "mgmt-mcp" }, id)
            .await
            .expect("delete");

        assert!(find_or_404(&db, id).await.is_err());

        let row = crate::entities::change_history::Entity::find()
            .one(&db)
            .await
            .expect("query")
            .expect("row exists");
        assert_eq!(
            (
                row.target_kind,
                row.target_id,
                row.actor_kind,
                row.actor_label,
                row.op,
            ),
            (
                "strategy".to_string(),
                id,
                "llm".to_string(),
                "mgmt-mcp".to_string(),
                "delete".to_string(),
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn delete_unknown_id_returns_not_found(pool: PgPool) {
        let db = create_test_db(pool).await;
        let err = delete(&db, Actor::Human, Uuid::new_v4()).await.unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    // 実際の並行リクエストは非決定的なため、confirm 済みの名前で呼ぶ delete_confirmed の
    // 直前に別経路で名前が変わることを、事前の rename で決定的に再現する。
    #[sqlx::test(migrations = false)]
    async fn delete_confirmed_rejects_when_name_changed_after_confirmation(pool: PgPool) {
        let db = create_test_db(pool).await;
        let id = insert_test_strategy(&db, "original").await;

        update(
            &db,
            Actor::Human,
            id,
            StrategyUpdate {
                name: Some("renamed".to_string()),
                ..Default::default()
            },
        )
        .await
        .expect("rename");

        let err = delete_confirmed(&db, Actor::Human, id, "original")
            .await
            .expect_err("stale confirm_name must be rejected");
        assert!(matches!(err, AppError::NotFound(_)));
        assert!(find_or_404(&db, id).await.is_ok());
    }
}
