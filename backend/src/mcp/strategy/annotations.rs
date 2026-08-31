//! アノテーション操作の inner method 実装。
//!
//! 戦略境界の検査は [`super::fetch_note_owned_by`] が担う。

use rmcp::ErrorData as McpError;
use rust_decimal::Decimal;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::entities::annotation;

use super::dto::{
    AnnotationDto, CreateAnnotationParams, CreateAnnotationResult, ReadAnnotationsParams,
    ReadAnnotationsResult,
};
use super::{
    ALLOWED_ANNOTATION_KINDS, DEFAULT_ANNOTATION_STATUS, STRATEGY_AGENT_ACTOR, StrategyServer,
    clamp_limit, db_error, decimal_to_f64, ensure_strategy_exists, fetch_note_owned_by,
    invalid_params,
};

fn f64_to_decimal(v: f64) -> Result<Decimal, McpError> {
    Decimal::try_from(v).map_err(|err| invalid_params(format!("invalid decimal value: {err}")))
}

fn annotation_to_dto(m: annotation::Model) -> AnnotationDto {
    AnnotationDto {
        annotation_id: m.id,
        strategy_id: m.strategy_id,
        target_symbol: m.target_symbol,
        target_kind: m.target_kind,
        timestamp: m.timestamp,
        price: m.price.map(decimal_to_f64),
        text: m.text,
        status: m.status,
        linked_note_id: m.linked_note_id,
        created_by_kind: m.created_by_kind,
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}

impl StrategyServer {
    pub(crate) async fn create_annotation_inner(
        &self,
        session_strategy_id: Uuid,
        params: CreateAnnotationParams,
    ) -> Result<CreateAnnotationResult, McpError> {
        let target_symbol = params.target_symbol.trim().to_string();
        if target_symbol.is_empty() {
            return Err(invalid_params("target_symbol must not be empty"));
        }
        if !ALLOWED_ANNOTATION_KINDS.contains(&params.target_kind.as_str()) {
            return Err(invalid_params(format!(
                "invalid target_kind: {} (expected one of {:?})",
                params.target_kind, ALLOWED_ANNOTATION_KINDS
            )));
        }
        if params.text.trim().is_empty() {
            return Err(invalid_params("text must not be empty"));
        }

        ensure_strategy_exists(&self.db, session_strategy_id).await?;

        // linked_note_id が指定されている場合、対象 note の strategy_id 一致を二重検査する
        if let Some(linked) = params.linked_note_id {
            fetch_note_owned_by(&self.db, linked, session_strategy_id).await?;
        }

        let price = params.price.map(f64_to_decimal).transpose()?;
        let id = Uuid::new_v4();
        let model = annotation::ActiveModel {
            id: Set(id),
            strategy_id: Set(session_strategy_id),
            target_symbol: Set(target_symbol),
            target_kind: Set(params.target_kind),
            timestamp: Set(params.timestamp),
            price: Set(price),
            text: Set(params.text),
            status: Set(DEFAULT_ANNOTATION_STATUS.to_string()),
            linked_note_id: Set(params.linked_note_id),
            created_by_kind: Set(STRATEGY_AGENT_ACTOR.to_string()),
            created_at: NotSet,
            updated_at: NotSet,
        };
        let created = annotation::Entity::insert(model)
            .exec_with_returning(&self.db)
            .await
            .map_err(db_error)?;
        Ok(CreateAnnotationResult {
            annotation: annotation_to_dto(created),
        })
    }

    pub(crate) async fn read_annotations_inner(
        &self,
        session_strategy_id: Uuid,
        params: ReadAnnotationsParams,
    ) -> Result<ReadAnnotationsResult, McpError> {
        let mut q = annotation::Entity::find()
            .filter(annotation::Column::StrategyId.eq(session_strategy_id))
            .order_by_desc(annotation::Column::Timestamp);
        if let Some(sym) = params
            .target_symbol
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            q = q.filter(annotation::Column::TargetSymbol.eq(sym));
        }
        let rows = q
            .limit(clamp_limit(params.limit))
            .all(&self.db)
            .await
            .map_err(db_error)?;
        Ok(ReadAnnotationsResult {
            annotations: rows.into_iter().map(annotation_to_dto).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset};
    use sqlx::PgPool;

    use crate::testing::create_test_db;

    use super::super::dto::{
        AnnotationDto, CreateAnnotationParams, ReadAnnotationsParams, ReadAnnotationsResult,
    };
    use super::super::tests_common::{
        build_server, insert_strategy, normalize_annotation, seed_foreign_note, ts_sentinel,
    };
    use super::super::{DEFAULT_ANNOTATION_STATUS, STRATEGY_AGENT_ACTOR};

    #[sqlx::test(migrations = false)]
    async fn create_annotation_then_read_annotations(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db);
        let ts: DateTime<FixedOffset> = "2026-06-01T09:00:00+09:00".parse().expect("ts");

        let created = server
            .create_annotation_inner(
                strategy_id,
                CreateAnnotationParams {
                    target_symbol: "7203".into(),
                    target_kind: "signal".into(),
                    timestamp: ts,
                    price: Some(25000.0),
                    text: "breakout".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect("create");
        let expected = AnnotationDto {
            annotation_id: created.annotation.annotation_id,
            strategy_id,
            target_symbol: "7203".into(),
            target_kind: "signal".into(),
            timestamp: ts,
            price: Some(25000.0),
            text: "breakout".into(),
            status: DEFAULT_ANNOTATION_STATUS.into(),
            linked_note_id: None,
            created_by_kind: STRATEGY_AGENT_ACTOR.into(),
            created_at: ts_sentinel(),
            updated_at: ts_sentinel(),
        };
        assert_eq!(normalize_annotation(created.annotation), expected);

        let list = server
            .read_annotations_inner(
                strategy_id,
                ReadAnnotationsParams {
                    target_symbol: None,
                    limit: None,
                },
            )
            .await
            .expect("list");
        assert_eq!(
            ReadAnnotationsResult {
                annotations: list
                    .annotations
                    .into_iter()
                    .map(normalize_annotation)
                    .collect(),
            },
            ReadAnnotationsResult {
                annotations: vec![expected],
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn create_annotation_rejects_invalid_kind(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let server = build_server(db);
        let err = server
            .create_annotation_inner(
                strategy_id,
                CreateAnnotationParams {
                    target_symbol: "7203".into(),
                    target_kind: "garbage".into(),
                    timestamp: "2026-06-01T00:00:00Z".parse().expect("ts"),
                    price: None,
                    text: "x".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect_err("invalid kind expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn create_annotation_rejects_cross_strategy_linked_note(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());
        let foreign_note = seed_foreign_note(&db, strategy_b, "b").await;

        let err = server
            .create_annotation_inner(
                strategy_a,
                CreateAnnotationParams {
                    target_symbol: "7203".into(),
                    target_kind: "signal".into(),
                    timestamp: "2026-06-01T00:00:00Z".parse().expect("ts"),
                    price: None,
                    text: "x".into(),
                    linked_note_id: Some(foreign_note),
                },
            )
            .await
            .expect_err("cross-strategy linked note expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}
