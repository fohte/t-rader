use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::AppState;
use crate::entities::{news_item, news_strategy_link, strategy};
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonPath;

/// 1 戦略あたりの返却件数上限。蓄積が長期化しても応答が爆発しないように上限を切る。
const MAX_NEWS_PER_STRATEGY: u64 = 50;

/// 戦略ホームの「関連ニュース」セクション用 1 件
#[derive(Debug, Serialize, ToSchema)]
pub struct StrategyNewsItem {
    pub id: Uuid,
    pub source: String,
    pub url: String,
    pub title: String,
    pub body_snippet: Option<String>,
    pub published_at: DateTime<Utc>,
    /// この戦略の interest のうち、このニュースに紐付いたものの一覧 (ref_kind:ref_id)
    pub matched_refs: Vec<MatchedRef>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MatchedRef {
    pub ref_kind: String,
    pub ref_id: String,
    pub matched_term: String,
}

/// 戦略に関連付けられたニュース一覧を取得する
///
/// `news_strategy_link` を介して `strategy_id` で絞り込み、`published_at` 降順で返す。
/// 他戦略のニュースは含まれない (link 経由のためテーブル境界で隔離される)。
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/news",
    tag = "news",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, description = "関連ニュース", body = Vec<StrategyNewsItem>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategy_news(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<Vec<StrategyNewsItem>>, AppError> {
    // 戦略の存在確認 (link が 0 件でも 404 と 200(empty) を区別する)
    let exists = strategy::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .is_some();
    if !exists {
        return Err(AppError::NotFound(format!("strategy {id} not found")));
    }

    // 戦略 link が付いているニュースのうち、新しい順に最大 MAX_NEWS_PER_STRATEGY 件だけ取り出す。
    // distinct を打って同一 news_id が複数 link に紐づくケースで件数を膨らませない。
    let news_rows = news_item::Entity::find()
        .inner_join(news_strategy_link::Entity)
        .filter(news_strategy_link::Column::StrategyId.eq(id))
        .order_by_desc(news_item::Column::PublishedAt)
        .distinct()
        .limit(MAX_NEWS_PER_STRATEGY)
        .all(&state.db)
        .await?;

    if news_rows.is_empty() {
        return Ok(Json(Vec::new()));
    }

    let news_ids: Vec<Uuid> = news_rows.iter().map(|n| n.id).collect();
    let links = news_strategy_link::Entity::find()
        .filter(news_strategy_link::Column::StrategyId.eq(id))
        .filter(news_strategy_link::Column::NewsId.is_in(news_ids))
        // matched_refs の順序を deterministic にする (created_at 昇順 → 同時刻なら ref_kind / ref_id)
        .order_by_asc(news_strategy_link::Column::CreatedAt)
        .order_by_asc(news_strategy_link::Column::RefKind)
        .order_by_asc(news_strategy_link::Column::RefId)
        .all(&state.db)
        .await?;

    // news_id -> matched_refs にまとめる
    let mut refs_by_news: HashMap<Uuid, Vec<MatchedRef>> = HashMap::new();
    for link in links {
        refs_by_news
            .entry(link.news_id)
            .or_default()
            .push(MatchedRef {
                ref_kind: link.ref_kind,
                ref_id: link.ref_id,
                matched_term: link.matched_term,
            });
    }

    let out: Vec<StrategyNewsItem> = news_rows
        .into_iter()
        .map(|n| StrategyNewsItem {
            id: n.id,
            source: n.source,
            url: n.url,
            title: n.title,
            body_snippet: n.body_snippet,
            published_at: n.published_at.with_timezone(&Utc),
            matched_refs: refs_by_news.remove(&n.id).unwrap_or_default(),
        })
        .collect();

    Ok(Json(out))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use sea_orm::{ActiveModelTrait, ActiveValue::NotSet, DatabaseConnection, Set};
    use serde_json::Value;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::AppState;
    use crate::entities::sea_orm_active_enums::StrategyAgentStatus;
    use crate::entities::{news_item, news_strategy_link, strategy};
    use crate::testing::create_test_server_with_db_and_kube;

    async fn insert_strategy(db: &DatabaseConnection, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        strategy::ActiveModel {
            id: Set(id),
            name: Set(name.into()),
            description: Set(None),
            sort_order: Set(0),
            agents_md: NotSet,
            skills: NotSet,
            agent_status: Set(StrategyAgentStatus::Ready),
            agent_error: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert strategy");
        id
    }

    async fn insert_news(
        db: &DatabaseConnection,
        title: &str,
        url: &str,
        published_offset_sec: i64,
    ) -> Uuid {
        let id = Uuid::new_v4();
        let published_at = Utc::now() - chrono::Duration::seconds(published_offset_sec);
        news_item::ActiveModel {
            id: Set(id),
            source: Set("Test".into()),
            url: Set(url.into()),
            title: Set(title.into()),
            body_snippet: Set(None),
            published_at: Set(published_at.into()),
            fetched_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .expect("insert news");
        id
    }

    async fn insert_link(
        db: &DatabaseConnection,
        news_id: Uuid,
        strategy_id: Uuid,
        ref_kind: &str,
        ref_id: &str,
        term: &str,
    ) {
        news_strategy_link::ActiveModel {
            news_id: Set(news_id),
            strategy_id: Set(strategy_id),
            ref_kind: Set(ref_kind.into()),
            ref_id: Set(ref_id.into()),
            matched_term: Set(term.into()),
            created_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .expect("insert link");
    }

    /// レスポンスの dynamic フィールド (id, published_at) を固定値に正規化する
    fn normalize_news_response(mut value: Value) -> Value {
        if let Some(items) = value.as_array_mut() {
            for item in items {
                if let Some(id) = item.get_mut("id") {
                    *id = serde_json::json!("NORMALIZED_ID");
                }
                if let Some(pa) = item.get_mut("published_at") {
                    *pa = serde_json::json!("NORMALIZED_TIME");
                }
            }
        }
        value
    }

    #[sqlx::test(migrations = false)]
    async fn list_strategy_news_returns_only_linked_news(pool: PgPool) {
        let (db, server) =
            create_test_server_with_db_and_kube(pool, AppState::disabled_kubeopencode()).await;

        let strategy_a = insert_strategy(&db, "戦略A").await;
        let strategy_b = insert_strategy(&db, "戦略B").await;

        let news1 = insert_news(&db, "トヨタ自動車 通期決算", "https://ex.com/1", 60).await;
        let news2 = insert_news(&db, "半導体テーマ上昇", "https://ex.com/2", 30).await;
        let news3 = insert_news(&db, "戦略B 向け", "https://ex.com/3", 10).await;

        insert_link(&db, news1, strategy_a, "stock", "7203", "トヨタ自動車").await;
        insert_link(&db, news2, strategy_a, "theme", "semiconductor", "半導体").await;
        insert_link(&db, news3, strategy_b, "theme", "x", "戦略B 向け").await;

        let res = server
            .get(&format!("/api/strategies/{strategy_a}/news"))
            .await;
        res.assert_status_ok();
        // 戦略 A に紐づく 2 件のみ。news3 (戦略 B) は他戦略 link なので絶対に混ざらない。
        // published_at desc なので news2 (30s 前) → news1 (60s 前) の順。
        assert_eq!(
            normalize_news_response(res.json::<Value>()),
            serde_json::json!([
                {
                    "id": "NORMALIZED_ID",
                    "source": "Test",
                    "url": "https://ex.com/2",
                    "title": "半導体テーマ上昇",
                    "body_snippet": null,
                    "published_at": "NORMALIZED_TIME",
                    "matched_refs": [
                        {
                            "ref_kind": "theme",
                            "ref_id": "semiconductor",
                            "matched_term": "半導体",
                        }
                    ],
                },
                {
                    "id": "NORMALIZED_ID",
                    "source": "Test",
                    "url": "https://ex.com/1",
                    "title": "トヨタ自動車 通期決算",
                    "body_snippet": null,
                    "published_at": "NORMALIZED_TIME",
                    "matched_refs": [
                        {
                            "ref_kind": "stock",
                            "ref_id": "7203",
                            "matched_term": "トヨタ自動車",
                        }
                    ],
                },
            ]),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn list_strategy_news_returns_404_for_nonexistent_strategy(pool: PgPool) {
        let (_, server) =
            create_test_server_with_db_and_kube(pool, AppState::disabled_kubeopencode()).await;
        let res = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000/news")
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn list_strategy_news_returns_empty_when_no_links(pool: PgPool) {
        let (db, server) =
            create_test_server_with_db_and_kube(pool, AppState::disabled_kubeopencode()).await;
        let strategy_id = insert_strategy(&db, "戦略X").await;
        let res = server
            .get(&format!("/api/strategies/{strategy_id}/news"))
            .await;
        res.assert_status_ok();
        assert_eq!(res.json::<Value>(), serde_json::json!([]));
    }

    #[sqlx::test(migrations = false)]
    async fn list_strategy_news_collapses_multiple_ref_matches_per_news(pool: PgPool) {
        let (db, server) =
            create_test_server_with_db_and_kube(pool, AppState::disabled_kubeopencode()).await;
        let strategy_id = insert_strategy(&db, "戦略M").await;
        let news_id = insert_news(&db, "トヨタ 半導体 双方", "https://ex.com/m", 5).await;
        insert_link(&db, news_id, strategy_id, "stock", "7203", "トヨタ").await;
        insert_link(&db, news_id, strategy_id, "theme", "semi", "半導体").await;

        let res = server
            .get(&format!("/api/strategies/{strategy_id}/news"))
            .await;
        res.assert_status_ok();
        // matched_refs は news_strategy_link の created_at 順だが、本テストでは insert を
        // 直列で実行するため stock → theme の順で固定される
        assert_eq!(
            normalize_news_response(res.json::<Value>()),
            serde_json::json!([{
                "id": "NORMALIZED_ID",
                "source": "Test",
                "url": "https://ex.com/m",
                "title": "トヨタ 半導体 双方",
                "body_snippet": null,
                "published_at": "NORMALIZED_TIME",
                "matched_refs": [
                    { "ref_kind": "stock", "ref_id": "7203", "matched_term": "トヨタ" },
                    { "ref_kind": "theme", "ref_id": "semi", "matched_term": "半導体" },
                ],
            }]),
        );
    }
}
