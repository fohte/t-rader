//! RSS フィードの CRUD service 層
//!
//! REST handler と MCP tool の両方からここを叩く。
//! 入力バリデーション (source slug 正規表現、URL 構文) もここで集約する。

use chrono::Utc;
use reqwest::Url;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel,
    QueryFilter, QueryOrder, RuntimeErr, SqlErr,
};
use uuid::Uuid;

use crate::entities::rss_feed;

/// `source` slug の許容文字集合の説明。machine key 用途 (`Bloomberg JP` 等の
/// display 文字列は display_name に入れる)。エラーメッセージで参照する
const SOURCE_PATTERN_DESC: &str = "^[a-z0-9_-]+$";

#[derive(Debug, thiserror::Error)]
pub enum RssFeedError {
    #[error("source must match {SOURCE_PATTERN_DESC} (got '{0}')")]
    InvalidSource(String),

    #[error("display_name must not be empty")]
    EmptyDisplayName,

    #[error("url must be a valid http(s) URL (got '{0}')")]
    InvalidUrl(String),

    #[error("rss feed with source '{0}' already exists")]
    DuplicateSource(String),

    #[error("rss feed {0} not found")]
    NotFound(Uuid),

    #[error("database error: {0}")]
    Database(#[from] DbErr),
}

#[derive(Debug, Clone)]
pub struct CreateInput {
    pub source: String,
    pub display_name: String,
    pub url: String,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdatePatch {
    pub display_name: Option<String>,
    pub url: Option<String>,
    pub enabled: Option<bool>,
}

fn validate_source(source: &str) -> Result<String, RssFeedError> {
    let trimmed = source.trim();
    if trimmed.is_empty()
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    {
        return Err(RssFeedError::InvalidSource(source.to_string()));
    }
    Ok(trimmed.to_string())
}

fn validate_url(url: &str) -> Result<String, RssFeedError> {
    let trimmed = url.trim();
    let parsed = Url::parse(trimmed).map_err(|_| RssFeedError::InvalidUrl(url.to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(RssFeedError::InvalidUrl(url.to_string()));
    }
    Ok(trimmed.to_string())
}

fn validate_display_name(name: &str) -> Result<String, RssFeedError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(RssFeedError::EmptyDisplayName);
    }
    Ok(trimmed.to_string())
}

/// `source` UNIQUE 違反 (Postgres 23505) を専用 variant に変換する
fn classify_insert_error(err: DbErr, source: &str) -> RssFeedError {
    if let Some(SqlErr::UniqueConstraintViolation(_)) = err.sql_err() {
        return RssFeedError::DuplicateSource(source.to_string());
    }
    // sql_err() で取りこぼすケースも raw SQLSTATE で念のため判定する
    if let DbErr::Exec(RuntimeErr::SqlxError(sqlx_err))
    | DbErr::Query(RuntimeErr::SqlxError(sqlx_err)) = &err
        && let Some(code) = sqlx_err.as_database_error().and_then(|e| e.code())
        && code.as_ref() == "23505"
    {
        return RssFeedError::DuplicateSource(source.to_string());
    }
    RssFeedError::Database(err)
}

/// 全件 (またはフィルタした) フィードを display_name 昇順で返す
pub async fn list(
    db: &DatabaseConnection,
    enabled_only: bool,
) -> Result<Vec<rss_feed::Model>, RssFeedError> {
    let mut query = rss_feed::Entity::find();
    if enabled_only {
        query = query.filter(rss_feed::Column::Enabled.eq(true));
    }
    let rows = query
        .order_by_asc(rss_feed::Column::DisplayName)
        .all(db)
        .await?;
    Ok(rows)
}

pub async fn create(
    db: &DatabaseConnection,
    input: CreateInput,
) -> Result<rss_feed::Model, RssFeedError> {
    let source = validate_source(&input.source)?;
    let display_name = validate_display_name(&input.display_name)?;
    let url = validate_url(&input.url)?;
    let model = rss_feed::ActiveModel {
        id: Set(Uuid::new_v4()),
        source: Set(source.clone()),
        display_name: Set(display_name),
        url: Set(url),
        enabled: Set(input.enabled.unwrap_or(true)),
        created_at: NotSet,
        updated_at: NotSet,
    };
    rss_feed::Entity::insert(model)
        .exec_with_returning(db)
        .await
        .map_err(|e| classify_insert_error(e, &source))
}

pub async fn update(
    db: &DatabaseConnection,
    id: Uuid,
    patch: UpdatePatch,
) -> Result<rss_feed::Model, RssFeedError> {
    let current = rss_feed::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(RssFeedError::NotFound(id))?;
    let mut active = current.into_active_model();
    if let Some(name) = patch.display_name {
        active.display_name = Set(validate_display_name(&name)?);
    }
    if let Some(url) = patch.url {
        active.url = Set(validate_url(&url)?);
    }
    if let Some(enabled) = patch.enabled {
        active.enabled = Set(enabled);
    }
    active.updated_at = Set(Utc::now().fixed_offset());
    Ok(active.update(db).await?)
}

pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<(), RssFeedError> {
    let result = rss_feed::Entity::delete_by_id(id).exec(db).await?;
    if result.rows_affected == 0 {
        return Err(RssFeedError::NotFound(id));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde_json::{Value, json};
    use sqlx::PgPool;

    use crate::entities::rss_feed;
    use crate::testing::create_test_db;

    /// Model 全体を JSON 化して dynamic フィールド (id/created_at/updated_at) を
    /// placeholder に潰す。CLAUDE.md の「全体を 1 度の equality で検証」を満たすため
    fn normalize(model: rss_feed::Model) -> Value {
        let mut v = serde_json::to_value(model).expect("model serializes");
        for key in ["id", "created_at", "updated_at"] {
            if let Some(slot) = v.get_mut(key) {
                *slot = Value::String(format!("<{key}>"));
            }
        }
        v
    }

    fn input(source: &str, name: &str, url: &str) -> CreateInput {
        CreateInput {
            source: source.into(),
            display_name: name.into(),
            url: url.into(),
            enabled: None,
        }
    }

    #[rstest]
    #[case::ok("bloomberg-jp")]
    #[case::with_underscore("yahoo_finance")]
    #[case::digits("nikkei-225")]
    fn validate_source_accepts(#[case] source: &str) {
        assert_eq!(validate_source(source).unwrap(), source);
    }

    #[rstest]
    #[case::uppercase("Bloomberg")]
    #[case::space("yahoo finance")]
    #[case::japanese("日経")]
    #[case::empty("")]
    fn validate_source_rejects(#[case] source: &str) {
        assert!(matches!(
            validate_source(source),
            Err(RssFeedError::InvalidSource(_))
        ));
    }

    #[rstest]
    #[case::http("http://example.com/feed.xml")]
    #[case::https("https://feeds.example.com/rss")]
    fn validate_url_accepts(#[case] url: &str) {
        assert_eq!(validate_url(url).unwrap(), url);
    }

    #[rstest]
    #[case::scheme_missing("example.com/feed")]
    #[case::ftp("ftp://example.com/feed")]
    #[case::garbage("not-a-url")]
    fn validate_url_rejects(#[case] url: &str) {
        assert!(matches!(
            validate_url(url),
            Err(RssFeedError::InvalidUrl(_))
        ));
    }

    #[sqlx::test(migrations = false)]
    async fn create_and_list_roundtrip(pool: PgPool) {
        let db = create_test_db(pool).await;
        let created = create(
            &db,
            input("bloomberg", "Bloomberg", "https://example.com/a"),
        )
        .await
        .unwrap();
        let expected = json!({
            "id": "<id>",
            "source": "bloomberg",
            "display_name": "Bloomberg",
            "url": "https://example.com/a",
            "enabled": true,
            "created_at": "<created_at>",
            "updated_at": "<updated_at>",
        });
        assert_eq!(normalize(created), expected);
        let listed = list(&db, false).await.unwrap();
        assert_eq!(
            listed.into_iter().map(normalize).collect::<Vec<_>>(),
            vec![expected],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn list_filters_by_enabled(pool: PgPool) {
        let db = create_test_db(pool).await;
        create(&db, input("a", "A", "https://example.com/a"))
            .await
            .unwrap();
        let b = create(&db, input("b", "B", "https://example.com/b"))
            .await
            .unwrap();
        update(
            &db,
            b.id,
            UpdatePatch {
                enabled: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let enabled = list(&db, true).await.unwrap();
        assert_eq!(
            enabled.into_iter().map(normalize).collect::<Vec<_>>(),
            vec![json!({
                "id": "<id>",
                "source": "a",
                "display_name": "A",
                "url": "https://example.com/a",
                "enabled": true,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            })],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn create_rejects_duplicate_source(pool: PgPool) {
        let db = create_test_db(pool).await;
        create(&db, input("dup", "Dup", "https://example.com/a"))
            .await
            .unwrap();
        let err = create(&db, input("dup", "Dup2", "https://example.com/b"))
            .await
            .unwrap_err();
        assert!(matches!(err, RssFeedError::DuplicateSource(s) if s == "dup"));
    }

    #[sqlx::test(migrations = false)]
    async fn create_rejects_invalid_source_slug(pool: PgPool) {
        let db = create_test_db(pool).await;
        let err = create(&db, input("Bad Source", "x", "https://example.com/a"))
            .await
            .unwrap_err();
        assert!(matches!(err, RssFeedError::InvalidSource(_)));
    }

    #[sqlx::test(migrations = false)]
    async fn create_rejects_invalid_url(pool: PgPool) {
        let db = create_test_db(pool).await;
        let err = create(&db, input("ok", "x", "not-a-url"))
            .await
            .unwrap_err();
        assert!(matches!(err, RssFeedError::InvalidUrl(_)));
    }

    #[sqlx::test(migrations = false)]
    async fn update_partial_patches_only_provided_fields(pool: PgPool) {
        let db = create_test_db(pool).await;
        let created = create(&db, input("src", "Old", "https://example.com/old"))
            .await
            .unwrap();
        let updated = update(
            &db,
            created.id,
            UpdatePatch {
                display_name: Some("New".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(
            normalize(updated),
            json!({
                "id": "<id>",
                "source": "src",
                "display_name": "New",
                "url": "https://example.com/old",
                "enabled": true,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn delete_removes_row(pool: PgPool) {
        let db = create_test_db(pool).await;
        let created = create(&db, input("src", "x", "https://example.com/a"))
            .await
            .unwrap();
        delete(&db, created.id).await.unwrap();
        assert!(list(&db, false).await.unwrap().is_empty());
    }

    #[sqlx::test(migrations = false)]
    async fn delete_missing_returns_not_found(pool: PgPool) {
        let db = create_test_db(pool).await;
        let err = delete(&db, Uuid::new_v4()).await.unwrap_err();
        assert!(matches!(err, RssFeedError::NotFound(_)));
    }
}
