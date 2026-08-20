//! 戦略 1 行の設定 (name / description / sort_order / agents_md / skills / agent_graph) の
//! 取得・作成・部分更新の共通経路。
//!
//! REST (`handlers::strategies`) と管理 MCP (`mcp::mgmt`) の両方から同じカラムへの書き込みが
//! 発生しうるため、検証・DB 更新・change_history 記録をここに集約し、書き込み経路を 1 つに
//! 保つ。agent_graph 固有の YAML 検証は `services::agent_graph` に残し、DB 更新のみここに委譲する。

use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ActiveModelTrait, DatabaseConnection, EntityTrait, IntoActiveModel, TransactionTrait,
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

pub fn validate_skill_name(name: &str) -> Result<(), AppError> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err(AppError::Validation("skill name must not be empty".into()));
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return Err(AppError::Validation(
            "skill name must start with [a-z0-9]".into(),
        ));
    }
    for c in chars {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-') {
            return Err(AppError::Validation(
                "skill name must match ^[a-z0-9][a-z0-9_-]*$".into(),
            ));
        }
    }
    Ok(())
}

pub fn skills_object(value: &serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    value
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new)
}

/// JSON Merge Patch (RFC 7396) 相当のセマンティクスで skills をマージする。patch の値が
/// null のキーは削除し、それ以外は追加/更新する。DB には触らない純粋関数。
pub fn apply_skills_patch(
    current: &serde_json::Value,
    patch: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut map = skills_object(current);
    for (k, v) in patch {
        if v.is_null() {
            map.remove(&k);
        } else {
            map.insert(k, v);
        }
    }
    map
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
        agents_md: NotSet,
        skills: NotSet,
        agent_graph: NotSet,
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
        json!({ "name": name, "sort_order": params.sort_order }),
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

pub async fn save_agents_md(
    db: &DatabaseConnection,
    actor: Actor,
    id: Uuid,
    content: String,
) -> Result<String, AppError> {
    let current = find_or_404(db, id).await?;
    let prev = current.agents_md.clone();
    let mut active = current.into_active_model();
    active.agents_md = Set(content.clone());
    active.updated_at = Set(chrono::Utc::now().fixed_offset());

    let txn = db.begin().await?;
    let updated = active.update(&txn).await?;
    change_history::record_as(
        &txn,
        actor,
        TargetKind::Strategy,
        id,
        Op::Update,
        json!({ "agents_md": { "from": prev, "to": content } }),
        Some("updated agents_md".to_string()),
    )
    .await?;
    txn.commit().await?;

    Ok(updated.agents_md)
}

pub async fn save_skills(
    db: &DatabaseConnection,
    actor: Actor,
    current: strategy::Model,
    skills: serde_json::Value,
    op_desc: String,
) -> Result<strategy::Model, AppError> {
    let id = current.id;
    let prev_skills = current.skills.clone();
    let mut active = current.into_active_model();
    active.skills = Set(skills.clone());
    active.updated_at = Set(chrono::Utc::now().fixed_offset());

    let txn = db.begin().await?;
    let updated = active.update(&txn).await?;
    change_history::record_as(
        &txn,
        actor,
        TargetKind::Strategy,
        id,
        Op::Update,
        json!({ "skills": { "from": prev_skills, "to": skills } }),
        Some(op_desc),
    )
    .await?;
    txn.commit().await?;

    Ok(updated)
}

/// 単一 skill を削除する。存在しない skill 名を指定した場合は `NotFound` を返す。
pub async fn delete_skill(
    db: &DatabaseConnection,
    actor: Actor,
    id: Uuid,
    name: &str,
) -> Result<strategy::Model, AppError> {
    let current = find_or_404(db, id).await?;
    let mut map = skills_object(&current.skills);
    if map.remove(name).is_none() {
        return Err(AppError::NotFound(format!("skill {name} not found")));
    }
    save_skills(
        db,
        actor,
        current,
        serde_json::Value::Object(map),
        format!("deleted skill {name}"),
    )
    .await
}

/// agent_graph カラムの保存 (検証は `services::agent_graph` が事前に済ませる前提)。
pub async fn save_agent_graph(
    db: &DatabaseConnection,
    actor: Actor,
    id: Uuid,
    content: String,
) -> Result<String, AppError> {
    let current = find_or_404(db, id).await?;
    let prev = current.agent_graph.clone();
    let mut active = current.into_active_model();
    active.agent_graph = Set(content.clone());
    active.updated_at = Set(chrono::Utc::now().fixed_offset());

    let txn = db.begin().await?;
    let updated = active.update(&txn).await?;
    change_history::record_as(
        &txn,
        actor,
        TargetKind::Strategy,
        id,
        Op::Update,
        json!({ "agent_graph": { "from": prev, "to": content } }),
        Some("updated agent_graph".to_string()),
    )
    .await?;
    txn.commit().await?;

    Ok(updated.agent_graph)
}

#[cfg(test)]
mod tests {
    use axum::response::IntoResponse;
    use sea_orm::EntityTrait;
    use serde_json::json;
    use sqlx::PgPool;

    use super::*;
    use crate::testing::{create_test_db, insert_test_strategy};

    // 実際の並行リクエストは非決定的なため、read (find_or_404) と update (save_skills) の
    // 間に別経路で削除を挟むことで、本来並行削除が起こるタイミングを決定的に再現する。
    #[sqlx::test(migrations = false)]
    async fn save_skills_returns_404_when_strategy_deleted_concurrently(pool: PgPool) {
        let db = create_test_db(pool).await;
        let id = insert_test_strategy(&db, "s").await;

        let current = save_skills(
            &db,
            Actor::Human,
            find_or_404(&db, id).await.expect("find strategy"),
            json!({ "scout": "first" }),
            "added skill scout".to_string(),
        )
        .await
        .expect("save skills");

        strategy::Entity::delete_by_id(id)
            .exec(&db)
            .await
            .expect("delete strategy");

        let mut map = skills_object(&current.skills);
        map.remove("scout");
        let err = save_skills(
            &db,
            Actor::Human,
            current,
            serde_json::Value::Object(map),
            "deleted skill scout".to_string(),
        )
        .await
        .expect_err("update against a deleted row must fail");

        let response = err.into_response();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let body: serde_json::Value = serde_json::from_slice(&bytes).expect("parse json body");
        assert_eq!(
            (status, body),
            (
                axum::http::StatusCode::NOT_FOUND,
                json!({ "error": "resource not found" }),
            )
        );
    }

    #[sqlx::test(migrations = false)]
    async fn delete_skill_returns_404_for_unknown_skill(pool: PgPool) {
        let db = create_test_db(pool).await;
        let id = insert_test_strategy(&db, "s").await;

        let err = delete_skill(&db, Actor::Human, id, "missing")
            .await
            .expect_err("must fail");
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[test]
    fn apply_skills_patch_upserts_and_deletes_via_null() {
        let current = json!({ "scout": "old", "review": "keep" });
        let mut patch = serde_json::Map::new();
        patch.insert("scout".to_string(), json!("new"));
        patch.insert("review".to_string(), serde_json::Value::Null);
        patch.insert("added".to_string(), json!("v"));

        let merged = apply_skills_patch(&current, patch);
        assert_eq!(
            serde_json::Value::Object(merged),
            json!({ "scout": "new", "added": "v" }),
        );
    }
}
