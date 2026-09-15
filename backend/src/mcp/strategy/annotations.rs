//! アノテーション操作の inner method 実装。
//!
//! 戦略境界の検査は [`super::fetch_note_owned_by`] が担う。

use rmcp::ErrorData as McpError;
use rust_decimal::Decimal;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait};
use uuid::Uuid;

use crate::entities::annotation;

use super::dto::{
    AnnotationDto, CreateAnnotationParams, CreateAnnotationResult, ReadAnnotationsParams,
    ReadAnnotationsResult,
};
use super::{
    DEFAULT_ANNOTATION_STATUS, STRATEGY_AGENT_ACTOR, StrategyServer, clamp_limit, db_error,
    decimal_to_f64, ensure_strategy_exists, fetch_note_owned_by, internal_error, invalid_params,
};

fn f64_to_decimal(v: f64) -> Result<Decimal, McpError> {
    Decimal::try_from(v).map_err(|err| invalid_params(format!("invalid decimal value: {err}")))
}

/// `m.strategy_id` は呼び出し元が `session_strategy_id` で絞り込んだ行から来るため
/// 必ず `Some` になるはずだが、不変条件が壊れた場合に別 strategy の id を誤って
/// 返さないよう fail-loud にする。
fn annotation_to_dto(m: annotation::Model) -> Result<AnnotationDto, McpError> {
    let strategy_id = m.strategy_id.ok_or_else(|| {
        internal_error(format!(
            "annotation {} has no strategy_id despite session scoping",
            m.id
        ))
    })?;
    Ok(AnnotationDto {
        annotation_id: m.id,
        strategy_id,
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
    })
}

impl StrategyServer {
    pub(crate) async fn create_annotation_inner(
        &self,
        session_strategy_id: Uuid,
        execution_step_id: Option<Uuid>,
        execution_task_id: Option<String>,
        params: CreateAnnotationParams,
    ) -> Result<CreateAnnotationResult, McpError> {
        let target_symbol = params.target_symbol.trim().to_string();
        if target_symbol.is_empty() {
            return Err(invalid_params("target_symbol must not be empty"));
        }
        let target_kind = params.target_kind.trim().to_string();
        if target_kind.is_empty() {
            return Err(invalid_params("target_kind must not be empty"));
        }
        if params.text.trim().is_empty() {
            return Err(invalid_params("text must not be empty"));
        }

        ensure_strategy_exists(&self.db, session_strategy_id).await?;

        // linked_note_id が指定されている場合、対象 note の strategy_id 一致を検査する
        if let Some(linked) = params.linked_note_id {
            fetch_note_owned_by(&self.db, linked, session_strategy_id).await?;
        }

        let price = params.price.map(f64_to_decimal).transpose()?;
        let id = Uuid::new_v4();
        let model = annotation::ActiveModel {
            id: Set(id),
            strategy_id: Set(Some(session_strategy_id)),
            target_symbol: Set(target_symbol),
            target_kind: Set(target_kind),
            timestamp: Set(params.timestamp),
            price: Set(price),
            text: Set(params.text),
            status: Set(DEFAULT_ANNOTATION_STATUS.to_string()),
            linked_note_id: Set(params.linked_note_id),
            created_by_kind: Set(STRATEGY_AGENT_ACTOR.to_string()),
            created_at: NotSet,
            updated_at: NotSet,
            execution_step_id: Set(execution_step_id),
            execution_task_id: Set(execution_task_id.clone()),
        };

        let txn = self.db.begin().await.map_err(db_error)?;
        // resume で同じステップが新しい試行 (execution_task_id) から作り始めたとき、前の試行が
        // 作った未レビュー (unread) のアノテーションを置き換える。承認/却下済みのものは残す。
        // 1 ステップで複数件作るのは正当な動作のため、note のような UNIQUE ではなく削除で対応する。
        if let (Some(step_id), Some(task_id)) = (execution_step_id, execution_task_id.as_deref()) {
            annotation::Entity::delete_many()
                .filter(annotation::Column::StrategyId.eq(session_strategy_id))
                .filter(annotation::Column::ExecutionStepId.eq(step_id))
                .filter(annotation::Column::Status.eq(DEFAULT_ANNOTATION_STATUS))
                .filter(annotation::Column::ExecutionTaskId.ne(task_id))
                .exec(&txn)
                .await
                .map_err(db_error)?;
        }
        let created = annotation::Entity::insert(model)
            .exec_with_returning(&txn)
            .await
            .map_err(db_error)?;
        txn.commit().await.map_err(db_error)?;
        Ok(CreateAnnotationResult {
            annotation: annotation_to_dto(created)?,
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
            annotations: rows
                .into_iter()
                .map(annotation_to_dto)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset};
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::annotation;
    use crate::testing::create_test_db;

    use super::super::dto::{
        AnnotationDto, CreateAnnotationParams, ReadAnnotationsParams, ReadAnnotationsResult,
    };
    use super::super::tests_common::{
        build_server, insert_strategy, normalize_annotation, seed_foreign_note, ts_sentinel,
    };
    use super::super::{DEFAULT_ANNOTATION_STATUS, STRATEGY_AGENT_ACTOR};

    // target_kind に旧 allowlist 外の値を使い、DB の CHECK 制約撤去 (target_kind は自由記述) を回帰検出する
    #[sqlx::test(migrations = false)]
    async fn create_annotation_then_read_annotations(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db);
        let ts: DateTime<FixedOffset> = "2026-06-01T09:00:00+09:00".parse().expect("ts");

        let created = server
            .create_annotation_inner(
                strategy_id,
                None,
                None,
                CreateAnnotationParams {
                    target_symbol: "7203".into(),
                    target_kind: "custom-tag".into(),
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
            target_kind: "custom-tag".into(),
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
    async fn create_annotation_rejects_empty_target_kind(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let server = build_server(db);
        let err = server
            .create_annotation_inner(
                strategy_id,
                None,
                None,
                CreateAnnotationParams {
                    target_symbol: "7203".into(),
                    target_kind: "  ".into(),
                    timestamp: "2026-06-01T00:00:00Z".parse().expect("ts"),
                    price: None,
                    text: "x".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect_err("empty target_kind expected to be rejected");
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
                None,
                None,
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

    /// resume で同じステップの新しい試行 (execution_task_id が変わる) が create_annotation を
    /// 呼んだとき、前の試行が作った未レビュー (unread) のアノテーションは削除され、
    /// 新しい試行のものだけが残る。
    #[sqlx::test(migrations = false)]
    async fn create_annotation_replaces_unread_annotations_from_previous_attempt_of_same_step(
        pool: PgPool,
    ) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db);
        let step_id = Uuid::new_v4();
        let ts: DateTime<FixedOffset> = "2026-06-01T00:00:00Z".parse().expect("ts");

        server
            .create_annotation_inner(
                strategy_id,
                Some(step_id),
                Some("attempt-1".into()),
                CreateAnnotationParams {
                    target_symbol: "7203".into(),
                    target_kind: "signal".into(),
                    timestamp: ts,
                    price: None,
                    text: "first attempt".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect("create from first attempt");

        let second = server
            .create_annotation_inner(
                strategy_id,
                Some(step_id),
                Some("attempt-2".into()),
                CreateAnnotationParams {
                    target_symbol: "7203".into(),
                    target_kind: "signal".into(),
                    timestamp: ts,
                    price: None,
                    text: "second attempt".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect("create from second (resumed) attempt");

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
            list.annotations
                .into_iter()
                .map(|a| a.annotation_id)
                .collect::<Vec<_>>(),
            vec![second.annotation.annotation_id],
        );
    }

    /// 前の試行が作ったアノテーションでも、既にレビュー済み (unread 以外) のものは
    /// 新しい試行が来ても削除されず残る。
    #[sqlx::test(migrations = false)]
    async fn create_annotation_keeps_reviewed_annotations_from_previous_attempt(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db.clone());
        let step_id = Uuid::new_v4();
        let ts: DateTime<FixedOffset> = "2026-06-01T00:00:00Z".parse().expect("ts");

        let reviewed = server
            .create_annotation_inner(
                strategy_id,
                Some(step_id),
                Some("attempt-1".into()),
                CreateAnnotationParams {
                    target_symbol: "7203".into(),
                    target_kind: "signal".into(),
                    timestamp: ts,
                    price: None,
                    text: "already reviewed".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect("create from first attempt");
        annotation::ActiveModel {
            id: Set(reviewed.annotation.annotation_id),
            status: Set("approved".into()),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("mark approved");

        let second = server
            .create_annotation_inner(
                strategy_id,
                Some(step_id),
                Some("attempt-2".into()),
                CreateAnnotationParams {
                    target_symbol: "7203".into(),
                    target_kind: "signal".into(),
                    timestamp: ts,
                    price: None,
                    text: "second attempt".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect("create from second (resumed) attempt");

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
        let mut ids: Vec<Uuid> = list
            .annotations
            .into_iter()
            .map(|a| a.annotation_id)
            .collect();
        ids.sort();
        let mut expected = vec![
            reviewed.annotation.annotation_id,
            second.annotation.annotation_id,
        ];
        expected.sort();
        assert_eq!(ids, expected);
    }

    /// resume していない通常の実行 (同じ execution_task_id) で 1 ステップが複数件の
    /// アノテーションを作る動作はこれまでどおり全件残る。
    #[sqlx::test(migrations = false)]
    async fn create_annotation_keeps_multiple_annotations_from_the_same_attempt(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db);
        let step_id = Uuid::new_v4();
        let ts: DateTime<FixedOffset> = "2026-06-01T00:00:00Z".parse().expect("ts");

        for text in ["first", "second"] {
            server
                .create_annotation_inner(
                    strategy_id,
                    Some(step_id),
                    Some("attempt-1".into()),
                    CreateAnnotationParams {
                        target_symbol: "7203".into(),
                        target_kind: "signal".into(),
                        timestamp: ts,
                        price: None,
                        text: text.into(),
                        linked_note_id: None,
                    },
                )
                .await
                .unwrap_or_else(|e| panic!("create {text} failed: {e}"));
        }

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
        let mut texts: Vec<String> = list.annotations.into_iter().map(|a| a.text).collect();
        texts.sort();
        assert_eq!(texts, vec!["first".to_string(), "second".to_string()]);
    }
}
