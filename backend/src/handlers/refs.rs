//! 一級参照型 (stock / indicator / sector / theme) の検索・詳細・リンク解決

use axum::Json;
use axum::extract::State;
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::AppState;
use crate::entities::{indicator, sector, stock, theme};
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonPath, JsonQuery};
use crate::models::RefResolution;

/// LIKE のメタ文字 (`%` `_` `\`) を入力から除去する。
/// SeaORM の `like()` は ESCAPE 句を出さないため、エスケープではなく除去で対処する
/// (検索 UI のサジェストとして `%` をそのまま検索したいケースは現状想定しない)。
fn sanitize_like(value: &str) -> String {
    value
        .chars()
        .filter(|c| *c != '%' && *c != '_' && *c != '\\')
        .collect()
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchQuery {
    /// 部分一致クエリ。空のときは先頭から最大 50 件返す
    #[serde(default)]
    pub q: Option<String>,
}

/// stock 検索
#[utoipa::path(
    get,
    path = "/api/refs/stocks",
    tag = "refs",
    params(SearchQuery),
    responses(
        (status = 200, body = Vec<stock::Model>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_stocks(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<SearchQuery>,
) -> Result<Json<Vec<stock::Model>>, AppError> {
    let mut q = stock::Entity::find().order_by_asc(stock::Column::Id);
    if let Some(text) = params.q.as_deref().filter(|s| !s.is_empty()) {
        let like = format!("%{}%", sanitize_like(text));
        q = q.filter(
            Condition::any()
                .add(stock::Column::Id.like(&like))
                .add(stock::Column::Name.like(&like)),
        );
    }
    let items = q.limit(50).all(&state.db).await?;
    Ok(Json(items))
}

/// stock 詳細
#[utoipa::path(
    get,
    path = "/api/refs/stocks/{id}",
    tag = "refs",
    params(("id" = String, Path, description = "銘柄コード")),
    responses(
        (status = 200, body = stock::Model),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_stock(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<String>,
) -> Result<Json<stock::Model>, AppError> {
    let m = stock::Entity::find_by_id(id.clone())
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("stock {id} not found")))?;
    Ok(Json(m))
}

/// indicator 検索
#[utoipa::path(
    get,
    path = "/api/refs/indicators",
    tag = "refs",
    params(SearchQuery),
    responses(
        (status = 200, body = Vec<indicator::Model>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_indicators(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<SearchQuery>,
) -> Result<Json<Vec<indicator::Model>>, AppError> {
    let mut q = indicator::Entity::find().order_by_asc(indicator::Column::Id);
    if let Some(text) = params.q.as_deref().filter(|s| !s.is_empty()) {
        let like = format!("%{}%", sanitize_like(text));
        q = q.filter(
            Condition::any()
                .add(indicator::Column::Id.like(&like))
                .add(indicator::Column::Name.like(&like)),
        );
    }
    let items = q.limit(50).all(&state.db).await?;
    Ok(Json(items))
}

/// indicator 詳細
#[utoipa::path(
    get,
    path = "/api/refs/indicators/{id}",
    tag = "refs",
    params(("id" = String, Path, description = "指標 ID")),
    responses(
        (status = 200, body = indicator::Model),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_indicator(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<String>,
) -> Result<Json<indicator::Model>, AppError> {
    let m = indicator::Entity::find_by_id(id.clone())
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("indicator {id} not found")))?;
    Ok(Json(m))
}

/// sector 検索
#[utoipa::path(
    get,
    path = "/api/refs/sectors",
    tag = "refs",
    params(SearchQuery),
    responses(
        (status = 200, body = Vec<sector::Model>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_sectors(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<SearchQuery>,
) -> Result<Json<Vec<sector::Model>>, AppError> {
    let mut q = sector::Entity::find().order_by_asc(sector::Column::Id);
    if let Some(text) = params.q.as_deref().filter(|s| !s.is_empty()) {
        let like = format!("%{}%", sanitize_like(text));
        q = q.filter(
            Condition::any()
                .add(sector::Column::Id.like(&like))
                .add(sector::Column::Name.like(&like)),
        );
    }
    let items = q.limit(50).all(&state.db).await?;
    Ok(Json(items))
}

/// sector 詳細
#[utoipa::path(
    get,
    path = "/api/refs/sectors/{id}",
    tag = "refs",
    params(("id" = String, Path, description = "セクター ID")),
    responses(
        (status = 200, body = sector::Model),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_sector(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<String>,
) -> Result<Json<sector::Model>, AppError> {
    let m = sector::Entity::find_by_id(id.clone())
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("sector {id} not found")))?;
    Ok(Json(m))
}

/// theme 検索
#[utoipa::path(
    get,
    path = "/api/refs/themes",
    tag = "refs",
    params(SearchQuery),
    responses(
        (status = 200, body = Vec<theme::Model>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_themes(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<SearchQuery>,
) -> Result<Json<Vec<theme::Model>>, AppError> {
    let mut q = theme::Entity::find().order_by_asc(theme::Column::Id);
    if let Some(text) = params.q.as_deref().filter(|s| !s.is_empty()) {
        let like = format!("%{}%", sanitize_like(text));
        q = q.filter(
            Condition::any()
                .add(theme::Column::Id.like(&like))
                .add(theme::Column::Name.like(&like)),
        );
    }
    let items = q.limit(50).all(&state.db).await?;
    Ok(Json(items))
}

/// theme 詳細
#[utoipa::path(
    get,
    path = "/api/refs/themes/{id}",
    tag = "refs",
    params(("id" = String, Path, description = "テーマ ID")),
    responses(
        (status = 200, body = theme::Model),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_theme(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<String>,
) -> Result<Json<theme::Model>, AppError> {
    let m = theme::Entity::find_by_id(id.clone())
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("theme {id} not found")))?;
    Ok(Json(m))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ResolveQuery {
    /// `[[kind:id]]` 形式のリンクテキスト、またはカンマ区切りで複数指定
    pub link: String,
}

/// `[[kind:id]]` の参照解決。リンクテキストから表示名を引く。
///
/// `link=stock:7203,indicator:USDJPY` のようにカンマ区切りで複数渡せる。
/// 不一致のものは name = null で返す。
#[utoipa::path(
    get,
    path = "/api/refs/resolve",
    tag = "refs",
    params(ResolveQuery),
    responses(
        (status = 200, body = Vec<RefResolution>),
        (status = 400, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn resolve_refs(
    State(state): State<AppState>,
    JsonQuery(params): JsonQuery<ResolveQuery>,
) -> Result<Json<Vec<RefResolution>>, AppError> {
    const MAX_LINKS: usize = 200;

    // 入力順序を保つために (kind, id) リストを保持し、解決は kind 単位で 1 クエリにまとめる
    let mut requested: Vec<(String, String)> = Vec::new();
    for raw in params.link.split(',') {
        let Some((kind, id)) = raw.split_once(':') else {
            return Err(AppError::Validation(format!(
                "invalid link format: {raw} (expected kind:id)"
            )));
        };
        let kind = kind.trim();
        let id = id.trim();
        if kind.is_empty() || id.is_empty() {
            return Err(AppError::Validation(format!("invalid link: {raw}")));
        }
        if !["stock", "indicator", "sector", "theme"].contains(&kind) {
            return Err(AppError::Validation(format!(
                "unknown ref kind: {kind} (allowed: stock, indicator, sector, theme)"
            )));
        }
        requested.push((kind.to_string(), id.to_string()));
        if requested.len() > MAX_LINKS {
            return Err(AppError::Validation(format!(
                "too many links (max {MAX_LINKS})"
            )));
        }
    }

    let mut ids_by_kind: std::collections::HashMap<&'static str, Vec<String>> =
        std::collections::HashMap::new();
    for (kind, id) in &requested {
        let key: &'static str = match kind.as_str() {
            "stock" => "stock",
            "indicator" => "indicator",
            "sector" => "sector",
            "theme" => "theme",
            _ => unreachable!(),
        };
        ids_by_kind.entry(key).or_default().push(id.clone());
    }

    let mut names: std::collections::HashMap<(String, String), String> =
        std::collections::HashMap::new();
    if let Some(ids) = ids_by_kind.get("stock") {
        for m in stock::Entity::find()
            .filter(stock::Column::Id.is_in(ids.clone()))
            .all(&state.db)
            .await?
        {
            names.insert(("stock".into(), m.id.clone()), m.name);
        }
    }
    if let Some(ids) = ids_by_kind.get("indicator") {
        for m in indicator::Entity::find()
            .filter(indicator::Column::Id.is_in(ids.clone()))
            .all(&state.db)
            .await?
        {
            names.insert(("indicator".into(), m.id.clone()), m.name);
        }
    }
    if let Some(ids) = ids_by_kind.get("sector") {
        for m in sector::Entity::find()
            .filter(sector::Column::Id.is_in(ids.clone()))
            .all(&state.db)
            .await?
        {
            names.insert(("sector".into(), m.id.clone()), m.name);
        }
    }
    if let Some(ids) = ids_by_kind.get("theme") {
        for m in theme::Entity::find()
            .filter(theme::Column::Id.is_in(ids.clone()))
            .all(&state.db)
            .await?
        {
            names.insert(("theme".into(), m.id.clone()), m.name);
        }
    }

    let out: Vec<RefResolution> = requested
        .into_iter()
        .map(|(kind, id)| RefResolution {
            name: names.get(&(kind.clone(), id.clone())).cloned(),
            kind,
            id,
        })
        .collect();
    Ok(Json(out))
}
