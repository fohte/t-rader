//! agent_config (目的 (purpose) をキーとする AGENTS.md / skills / agent_graph) の CRUD service 層。
//!
//! REST handler から叩く。入力バリデーション (purpose / skill 名の slug 正規表現、
//! agent_graph の YAML 検証) もここで集約する。agent_graph の YAML パース自体は
//! `services::agent_graph` (戦略の agent_graph とも共用する汎用ロジック) に委譲する。
//!
//! `change_history::TargetKind` に対応する種別が無いため、変更前後の値は
//! `account_risk_policy` と同様に `tracing::info!` にのみ残す。

use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel,
    QueryFilter, QueryOrder, RuntimeErr, SqlErr,
};
use uuid::Uuid;

use crate::entities::agent_config;
use crate::services::agent_graph::{self as agent_graph_svc, AgentGraphError};
use crate::services::strategy_config::{apply_skills_patch, skills_object, skills_to_btree};

const SLUG_PATTERN_DESC: &str = "^[a-z0-9][a-z0-9_-]*$";

#[derive(Debug, thiserror::Error)]
pub enum AgentConfigError {
    #[error("purpose must match {SLUG_PATTERN_DESC} (got '{0}')")]
    InvalidPurpose(String),

    #[error("skill name must match {SLUG_PATTERN_DESC} (got '{0}')")]
    InvalidSkillName(String),

    #[error("agent_config with purpose '{0}' already exists")]
    DuplicatePurpose(String),

    #[error("agent_config '{0}' not found")]
    NotFound(String),

    #[error("skill '{0}' not found")]
    SkillNotFound(String),

    #[error(transparent)]
    InvalidAgentGraph(#[from] AgentGraphError),

    #[error("database error: {0}")]
    Database(#[from] DbErr),
}

fn is_valid_slug(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => chars
            .clone()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'),
        _ => false,
    }
}

fn validate_purpose(purpose: &str) -> Result<(), AgentConfigError> {
    if !is_valid_slug(purpose) {
        return Err(AgentConfigError::InvalidPurpose(purpose.to_string()));
    }
    Ok(())
}

fn validate_skill_name(name: &str) -> Result<(), AgentConfigError> {
    if !is_valid_slug(name) {
        return Err(AgentConfigError::InvalidSkillName(name.to_string()));
    }
    Ok(())
}

/// `purpose` UNIQUE 違反 (Postgres 23505) を専用 variant に変換する
fn classify_insert_error(err: DbErr, purpose: &str) -> AgentConfigError {
    if let Some(SqlErr::UniqueConstraintViolation(_)) = err.sql_err() {
        return AgentConfigError::DuplicatePurpose(purpose.to_string());
    }
    // sql_err() で取りこぼすケースも raw SQLSTATE で念のため判定する
    if let DbErr::Exec(RuntimeErr::SqlxError(sqlx_err))
    | DbErr::Query(RuntimeErr::SqlxError(sqlx_err)) = &err
        && let Some(code) = sqlx_err.as_database_error().and_then(|e| e.code())
        && code.as_ref() == "23505"
    {
        return AgentConfigError::DuplicatePurpose(purpose.to_string());
    }
    AgentConfigError::Database(err)
}

/// purpose 昇順で全件返す
pub async fn list(db: &DatabaseConnection) -> Result<Vec<agent_config::Model>, AgentConfigError> {
    Ok(agent_config::Entity::find()
        .order_by_asc(agent_config::Column::Purpose)
        .all(db)
        .await?)
}

pub async fn find_or_404(
    db: &DatabaseConnection,
    purpose: &str,
) -> Result<agent_config::Model, AgentConfigError> {
    agent_config::Entity::find()
        .filter(agent_config::Column::Purpose.eq(purpose))
        .one(db)
        .await?
        .ok_or_else(|| AgentConfigError::NotFound(purpose.to_string()))
}

pub async fn create(
    db: &DatabaseConnection,
    purpose: String,
) -> Result<agent_config::Model, AgentConfigError> {
    validate_purpose(&purpose)?;
    let model = agent_config::ActiveModel {
        id: Set(Uuid::new_v4()),
        purpose: Set(purpose.clone()),
        agents_md: NotSet,
        skills: NotSet,
        agent_graph: NotSet,
        created_at: NotSet,
        updated_at: NotSet,
    };
    let created = agent_config::Entity::insert(model)
        .exec_with_returning(db)
        .await
        .map_err(|e| classify_insert_error(e, &purpose))?;
    tracing::info!(purpose = %created.purpose, "created agent_config");
    Ok(created)
}

pub async fn delete(db: &DatabaseConnection, purpose: &str) -> Result<(), AgentConfigError> {
    let result = agent_config::Entity::delete_many()
        .filter(agent_config::Column::Purpose.eq(purpose))
        .exec(db)
        .await?;
    if result.rows_affected == 0 {
        return Err(AgentConfigError::NotFound(purpose.to_string()));
    }
    tracing::info!(purpose, "deleted agent_config");
    Ok(())
}

pub async fn save_agents_md(
    db: &DatabaseConnection,
    purpose: &str,
    content: String,
) -> Result<String, AgentConfigError> {
    let current = find_or_404(db, purpose).await?;
    let from_len = current.agents_md.len();
    let mut active = current.into_active_model();
    active.agents_md = Set(content);
    active.updated_at = Set(chrono::Utc::now().fixed_offset());
    let saved = active.update(db).await?.agents_md;
    tracing::info!(
        purpose,
        from_len,
        to_len = saved.len(),
        "updated agent_config agents_md"
    );
    Ok(saved)
}

async fn save_skills(
    db: &DatabaseConnection,
    current: agent_config::Model,
    skills: serde_json::Value,
) -> Result<agent_config::Model, AgentConfigError> {
    let purpose = current.purpose.clone();
    let from: Vec<String> = skills_to_btree(&current.skills).into_keys().collect();
    let to: Vec<String> = skills_to_btree(&skills).into_keys().collect();
    let mut active = current.into_active_model();
    active.skills = Set(skills);
    active.updated_at = Set(chrono::Utc::now().fixed_offset());
    let saved = active.update(db).await?;
    tracing::info!(purpose, ?from, ?to, "updated agent_config skills");
    Ok(saved)
}

/// skills 全置換
pub async fn put_skills(
    db: &DatabaseConnection,
    purpose: &str,
    skills: std::collections::BTreeMap<String, String>,
) -> Result<agent_config::Model, AgentConfigError> {
    for name in skills.keys() {
        validate_skill_name(name)?;
    }
    let current = find_or_404(db, purpose).await?;
    let map: serde_json::Map<String, serde_json::Value> = skills
        .into_iter()
        .map(|(k, v)| (k, serde_json::Value::String(v)))
        .collect();
    save_skills(db, current, serde_json::Value::Object(map)).await
}

/// 単一 skill の追加 / 更新
pub async fn put_skill(
    db: &DatabaseConnection,
    purpose: &str,
    name: &str,
    content: String,
) -> Result<agent_config::Model, AgentConfigError> {
    validate_skill_name(name)?;
    let current = find_or_404(db, purpose).await?;
    let mut patch = serde_json::Map::new();
    patch.insert(name.to_string(), serde_json::Value::String(content));
    let merged = apply_skills_patch(&current.skills, patch);
    save_skills(db, current, serde_json::Value::Object(merged)).await
}

/// 単一 skill の削除。存在しない skill 名を指定した場合は `SkillNotFound` を返す。
pub async fn delete_skill(
    db: &DatabaseConnection,
    purpose: &str,
    name: &str,
) -> Result<agent_config::Model, AgentConfigError> {
    let current = find_or_404(db, purpose).await?;
    let mut map = skills_object(&current.skills);
    if map.remove(name).is_none() {
        return Err(AgentConfigError::SkillNotFound(name.to_string()));
    }
    save_skills(db, current, serde_json::Value::Object(map)).await
}

/// agent_graph の YAML を検証した上で保存する。
pub async fn save_agent_graph(
    db: &DatabaseConnection,
    purpose: &str,
    content: &str,
) -> Result<String, AgentConfigError> {
    agent_graph_svc::parse_agent_graph(content)?;
    let current = find_or_404(db, purpose).await?;
    let from_len = current.agent_graph.len();
    let mut active = current.into_active_model();
    active.agent_graph = Set(content.to_string());
    active.updated_at = Set(chrono::Utc::now().fixed_offset());
    let saved = active.update(db).await?.agent_graph;
    tracing::info!(
        purpose,
        from_len,
        to_len = saved.len(),
        "updated agent_config agent_graph"
    );
    Ok(saved)
}

pub fn skills_as_btree(model: &agent_config::Model) -> std::collections::BTreeMap<String, String> {
    skills_to_btree(&model.skills)
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use rstest::rstest;
    use sqlx::PgPool;

    use super::*;
    use crate::testing::create_test_db;

    fn normalize(model: agent_config::Model) -> serde_json::Value {
        let mut v = serde_json::to_value(model).expect("model serializes");
        for key in ["id", "created_at", "updated_at"] {
            if let Some(slot) = v.get_mut(key) {
                *slot = serde_json::Value::String(format!("<{key}>"));
            }
        }
        v
    }

    #[rstest]
    #[case::ok("explore")]
    #[case::with_underscore("deep_dive")]
    #[case::digits("review-2")]
    fn is_valid_slug_accepts(#[case] value: &str) {
        assert!(is_valid_slug(value));
    }

    #[rstest]
    #[case::uppercase("Explore")]
    #[case::space("deep dive")]
    #[case::japanese("探索")]
    #[case::empty("")]
    #[case::leading_dash("-explore")]
    fn is_valid_slug_rejects(#[case] value: &str) {
        assert!(!is_valid_slug(value));
    }

    #[sqlx::test(migrations = false)]
    async fn create_and_list_roundtrip(pool: PgPool) {
        let db = create_test_db(pool).await;
        let created = create(&db, "explore".to_string()).await.unwrap();
        let expected = serde_json::json!({
            "id": "<id>",
            "purpose": "explore",
            "agents_md": "",
            "skills": {},
            "agent_graph": "",
            "created_at": "<created_at>",
            "updated_at": "<updated_at>",
        });
        assert_eq!(normalize(created), expected);
        let listed = list(&db).await.unwrap();
        assert_eq!(
            listed.into_iter().map(normalize).collect::<Vec<_>>(),
            vec![expected],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn create_rejects_duplicate_purpose(pool: PgPool) {
        let db = create_test_db(pool).await;
        create(&db, "explore".to_string()).await.unwrap();
        let err = create(&db, "explore".to_string()).await.unwrap_err();
        assert!(matches!(err, AgentConfigError::DuplicatePurpose(p) if p == "explore"));
    }

    #[sqlx::test(migrations = false)]
    async fn create_rejects_invalid_purpose_slug(pool: PgPool) {
        let db = create_test_db(pool).await;
        let err = create(&db, "Bad Purpose".to_string()).await.unwrap_err();
        assert!(matches!(err, AgentConfigError::InvalidPurpose(_)));
    }

    #[sqlx::test(migrations = false)]
    async fn find_or_404_rejects_unknown_purpose(pool: PgPool) {
        let db = create_test_db(pool).await;
        let err = find_or_404(&db, "missing").await.unwrap_err();
        assert!(matches!(err, AgentConfigError::NotFound(p) if p == "missing"));
    }

    #[sqlx::test(migrations = false)]
    async fn delete_removes_row(pool: PgPool) {
        let db = create_test_db(pool).await;
        create(&db, "explore".to_string()).await.unwrap();
        delete(&db, "explore").await.unwrap();
        assert!(list(&db).await.unwrap().is_empty());
    }

    #[sqlx::test(migrations = false)]
    async fn delete_missing_returns_not_found(pool: PgPool) {
        let db = create_test_db(pool).await;
        let err = delete(&db, "missing").await.unwrap_err();
        assert!(matches!(err, AgentConfigError::NotFound(_)));
    }

    #[sqlx::test(migrations = false)]
    async fn save_then_get_agents_md_round_trips(pool: PgPool) {
        let db = create_test_db(pool).await;
        create(&db, "explore".to_string()).await.unwrap();
        let content = "# 方針\n慎重に運用する";
        let saved = save_agents_md(&db, "explore", content.to_string())
            .await
            .unwrap();
        assert_eq!(saved, content);
        assert_eq!(
            find_or_404(&db, "explore").await.unwrap().agents_md,
            content
        );
    }

    #[sqlx::test(migrations = false)]
    async fn put_skills_replaces_whole_map(pool: PgPool) {
        let db = create_test_db(pool).await;
        create(&db, "explore".to_string()).await.unwrap();

        let mut skills = std::collections::BTreeMap::new();
        skills.insert("scout".to_string(), "scout body".to_string());
        let updated = put_skills(&db, "explore", skills).await.unwrap();
        assert_eq!(
            skills_as_btree(&updated),
            std::collections::BTreeMap::from([("scout".to_string(), "scout body".to_string())]),
        );

        let mut replacement = std::collections::BTreeMap::new();
        replacement.insert("only".to_string(), "left".to_string());
        let replaced = put_skills(&db, "explore", replacement).await.unwrap();
        assert_eq!(
            skills_as_btree(&replaced),
            std::collections::BTreeMap::from([("only".to_string(), "left".to_string())]),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn put_skills_rejects_invalid_name(pool: PgPool) {
        let db = create_test_db(pool).await;
        create(&db, "explore".to_string()).await.unwrap();
        let mut skills = std::collections::BTreeMap::new();
        skills.insert("Bad Name".to_string(), "x".to_string());
        let err = put_skills(&db, "explore", skills).await.unwrap_err();
        assert!(matches!(err, AgentConfigError::InvalidSkillName(_)));
    }

    #[sqlx::test(migrations = false)]
    async fn put_skill_add_update_delete_lifecycle(pool: PgPool) {
        let db = create_test_db(pool).await;
        create(&db, "explore".to_string()).await.unwrap();

        put_skill(&db, "explore", "scout", "first".to_string())
            .await
            .unwrap();
        put_skill(&db, "explore", "scout", "second".to_string())
            .await
            .unwrap();
        let after_add = put_skill(&db, "explore", "review", "rev".to_string())
            .await
            .unwrap();
        assert_eq!(
            skills_as_btree(&after_add),
            std::collections::BTreeMap::from([
                ("scout".to_string(), "second".to_string()),
                ("review".to_string(), "rev".to_string()),
            ]),
        );

        let after_delete = delete_skill(&db, "explore", "scout").await.unwrap();
        assert_eq!(
            skills_as_btree(&after_delete),
            std::collections::BTreeMap::from([("review".to_string(), "rev".to_string())]),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn delete_skill_rejects_unknown_skill(pool: PgPool) {
        let db = create_test_db(pool).await;
        create(&db, "explore".to_string()).await.unwrap();
        let err = delete_skill(&db, "explore", "missing").await.unwrap_err();
        assert!(matches!(err, AgentConfigError::SkillNotFound(name) if name == "missing"));
    }

    #[sqlx::test(migrations = false)]
    async fn save_then_get_agent_graph_round_trips(pool: PgPool) {
        let db = create_test_db(pool).await;
        create(&db, "explore".to_string()).await.unwrap();
        let yaml = indoc! {"
            phases:
              - key: plan
                label: 調査計画
                model: claude-opus-4
                prompt: 仮説を立てよ
        "};
        let saved = save_agent_graph(&db, "explore", yaml).await.unwrap();
        assert_eq!(saved, yaml);
        assert_eq!(find_or_404(&db, "explore").await.unwrap().agent_graph, yaml);
    }

    #[sqlx::test(migrations = false)]
    async fn save_agent_graph_rejects_invalid_yaml_and_leaves_row_unchanged(pool: PgPool) {
        let db = create_test_db(pool).await;
        create(&db, "explore".to_string()).await.unwrap();
        let err = save_agent_graph(&db, "explore", "phases: [")
            .await
            .unwrap_err();
        assert!(matches!(err, AgentConfigError::InvalidAgentGraph(_)));
        assert_eq!(find_or_404(&db, "explore").await.unwrap().agent_graph, "");
    }
}
