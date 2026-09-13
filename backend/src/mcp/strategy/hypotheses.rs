//! 仮説 (hypothesis) の読み取り / 変更提案 tool。
//!
//! 仮説はエージェントに直接書き換えさせない設計のため、書き込みは「提案」の永続化のみで
//! 仮説本体には反映しない。承認/却下と、承認時に仮説へ適用する処理は REST API 側 (別モジュール)
//! が担う。
//!
//! スコープは「自戦略の仮説」+「account-wide (global, `strategy_id IS NULL`) 仮説」の和集合。
//! `list_watch_targets_inner` (`super::interests`) と同じ考え方だが、他戦略専属の仮説への
//! アクセスは拒否する (`fetch_note_owned_by` 等と同様、対象が厳密に決まる読み取り/書き込みの
//! ため)。

use rmcp::ErrorData as McpError;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::entities::{hypothesis, hypothesis_proposal};
use crate::services::hypotheses::ensure_status;

use super::dto::{
    HypothesisDto, ListHypothesesParams, ListHypothesesResult, ProposeHypothesisChangeParams,
    ProposeHypothesisChangeResult, ReadHypothesisParams,
};
use super::{StrategyServer, clamp_limit, db_error, invalid_params};

const PENDING_PROPOSAL_STATUS: &str = "pending";

fn validation_to_mcp(err: crate::error::AppError) -> McpError {
    match err {
        crate::error::AppError::Validation(msg) => invalid_params(msg),
        other => invalid_params(format!("validation failed: {other}")),
    }
}

fn hypothesis_to_dto(m: hypothesis::Model) -> HypothesisDto {
    HypothesisDto {
        hypothesis_id: m.hypothesis_id,
        strategy_id: m.strategy_id,
        title: m.title,
        body: m.body,
        status: m.status,
        related_note_ids: m.related_note_ids,
        related_interest_ids: m.related_interest_ids,
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}

/// id で仮説を取得し、自戦略または global (`strategy_id IS NULL`) のものだけ通す。
async fn fetch_hypothesis_visible_to(
    db: &sea_orm::DatabaseConnection,
    hypothesis_id: Uuid,
    session_strategy_id: Uuid,
) -> Result<hypothesis::Model, McpError> {
    let row = hypothesis::Entity::find_by_id(hypothesis_id)
        .one(db)
        .await
        .map_err(db_error)?
        .ok_or_else(|| McpError::resource_not_found("hypothesis not found", None))?;
    if let Some(owner) = row.strategy_id
        && owner != session_strategy_id
    {
        return Err(invalid_params(format!(
            "forbidden: hypothesis {hypothesis_id} belongs to another strategy"
        )));
    }
    Ok(row)
}

impl StrategyServer {
    pub(crate) async fn list_hypotheses_inner(
        &self,
        session_strategy_id: Uuid,
        params: ListHypothesesParams,
    ) -> Result<ListHypothesesResult, McpError> {
        let rows = hypothesis::Entity::find()
            .filter(
                Condition::any()
                    .add(hypothesis::Column::StrategyId.eq(session_strategy_id))
                    .add(hypothesis::Column::StrategyId.is_null()),
            )
            .order_by_desc(hypothesis::Column::UpdatedAt)
            .limit(clamp_limit(params.limit))
            .all(&self.db)
            .await
            .map_err(db_error)?;
        Ok(ListHypothesesResult {
            hypotheses: rows.into_iter().map(hypothesis_to_dto).collect(),
        })
    }

    pub(crate) async fn read_hypothesis_inner(
        &self,
        session_strategy_id: Uuid,
        params: ReadHypothesisParams,
    ) -> Result<HypothesisDto, McpError> {
        let row = fetch_hypothesis_visible_to(&self.db, params.hypothesis_id, session_strategy_id)
            .await?;
        Ok(hypothesis_to_dto(row))
    }

    pub(crate) async fn propose_hypothesis_change_inner(
        &self,
        session_strategy_id: Uuid,
        params: ProposeHypothesisChangeParams,
    ) -> Result<ProposeHypothesisChangeResult, McpError> {
        fetch_hypothesis_visible_to(&self.db, params.hypothesis_id, session_strategy_id).await?;

        if params.rationale.trim().is_empty() {
            return Err(invalid_params("rationale must not be empty"));
        }
        if params.proposed_title.is_none()
            && params.proposed_body.is_none()
            && params.proposed_status.is_none()
        {
            return Err(invalid_params(
                "at least one of proposed_title, proposed_body, proposed_status must be provided",
            ));
        }
        if let Some(title) = &params.proposed_title
            && title.trim().is_empty()
        {
            return Err(invalid_params("proposed_title must not be empty"));
        }
        if let Some(body) = &params.proposed_body
            && body.trim().is_empty()
        {
            return Err(invalid_params("proposed_body must not be empty"));
        }
        if let Some(status) = &params.proposed_status {
            ensure_status(status).map_err(validation_to_mcp)?;
        }

        let model = hypothesis_proposal::ActiveModel {
            id: Set(Uuid::new_v4()),
            hypothesis_id: Set(params.hypothesis_id),
            proposed_title: Set(params.proposed_title),
            proposed_body: Set(params.proposed_body),
            proposed_status: Set(params.proposed_status),
            rationale: Set(params.rationale),
            status: Set(PENDING_PROPOSAL_STATUS.to_string()),
            review_note: Set(None),
            created_at: NotSet,
            reviewed_at: Set(None),
        };
        let created = hypothesis_proposal::Entity::insert(model)
            .exec_with_returning(&self.db)
            .await
            .map_err(db_error)?;
        Ok(ProposeHypothesisChangeResult {
            proposal_id: created.id,
            status: created.status,
        })
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::EntityTrait;
    use sqlx::PgPool;

    use crate::testing::create_test_db;

    use super::super::dto::{
        ListHypothesesParams, ProposeHypothesisChangeParams, ReadHypothesisParams,
    };
    use super::super::tests_common::{build_server, insert_strategy, seed_hypothesis};

    #[sqlx::test(migrations = false)]
    async fn list_hypotheses_returns_own_and_global_but_not_other_strategy(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());

        let own = seed_hypothesis(
            &db,
            Some(strategy_a),
            "own hypothesis",
            "body",
            "unverified",
        )
        .await;
        let global = seed_hypothesis(&db, None, "global hypothesis", "body", "unverified").await;
        seed_hypothesis(
            &db,
            Some(strategy_b),
            "other strategy hypothesis",
            "body",
            "unverified",
        )
        .await;

        let result = server
            .list_hypotheses_inner(strategy_a, ListHypothesesParams { limit: None })
            .await
            .expect("list_hypotheses");

        let mut ids: Vec<_> = result
            .hypotheses
            .into_iter()
            .map(|h| h.hypothesis_id)
            .collect();
        ids.sort();
        let mut expected = vec![own, global];
        expected.sort();
        assert_eq!(ids, expected);
    }

    #[sqlx::test(migrations = false)]
    async fn read_hypothesis_returns_own_hypothesis(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        let server = build_server(db.clone());
        let hypothesis_id = seed_hypothesis(
            &db,
            Some(strategy_id),
            "own hypothesis",
            "body text",
            "unverified",
        )
        .await;

        let result = server
            .read_hypothesis_inner(strategy_id, ReadHypothesisParams { hypothesis_id })
            .await
            .expect("read_hypothesis");

        assert_eq!(
            (
                result.hypothesis_id,
                result.strategy_id,
                result.title,
                result.body,
                result.status,
            ),
            (
                hypothesis_id,
                Some(strategy_id),
                "own hypothesis".to_string(),
                "body text".to_string(),
                "unverified".to_string(),
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn read_hypothesis_returns_global_hypothesis(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        let server = build_server(db.clone());
        let hypothesis_id =
            seed_hypothesis(&db, None, "global hypothesis", "body", "supported").await;

        let result = server
            .read_hypothesis_inner(strategy_id, ReadHypothesisParams { hypothesis_id })
            .await
            .expect("read_hypothesis");

        assert_eq!(
            (result.hypothesis_id, result.strategy_id, result.status),
            (hypothesis_id, None, "supported".to_string()),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn read_hypothesis_rejects_other_strategy_hypothesis(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());
        let hypothesis_id = seed_hypothesis(
            &db,
            Some(strategy_b),
            "b's hypothesis",
            "body",
            "unverified",
        )
        .await;

        let err = server
            .read_hypothesis_inner(strategy_a, ReadHypothesisParams { hypothesis_id })
            .await
            .expect_err("cross-strategy hypothesis expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn read_hypothesis_rejects_missing_hypothesis(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        let server = build_server(db);

        let err = server
            .read_hypothesis_inner(
                strategy_id,
                ReadHypothesisParams {
                    hypothesis_id: uuid::Uuid::new_v4(),
                },
            )
            .await
            .expect_err("missing hypothesis expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::RESOURCE_NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn propose_hypothesis_change_creates_pending_proposal_with_only_given_fields(
        pool: PgPool,
    ) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        let server = build_server(db.clone());
        let hypothesis_id = seed_hypothesis(
            &db,
            Some(strategy_id),
            "own hypothesis",
            "body",
            "unverified",
        )
        .await;

        let result = server
            .propose_hypothesis_change_inner(
                strategy_id,
                ProposeHypothesisChangeParams {
                    hypothesis_id,
                    proposed_title: None,
                    proposed_body: Some("revised body".into()),
                    proposed_status: Some("supported".into()),
                    rationale: "price action confirms the thesis".into(),
                },
            )
            .await
            .expect("propose_hypothesis_change");

        assert_eq!(result.status, "pending".to_string());

        let stored = crate::entities::hypothesis_proposal::Entity::find_by_id(result.proposal_id)
            .one(&db)
            .await
            .expect("query proposal")
            .expect("proposal exists");
        assert_eq!(
            (
                stored.hypothesis_id,
                stored.proposed_title,
                stored.proposed_body,
                stored.proposed_status,
                stored.rationale,
                stored.status,
                stored.review_note,
                stored.reviewed_at,
            ),
            (
                hypothesis_id,
                None,
                Some("revised body".to_string()),
                Some("supported".to_string()),
                "price action confirms the thesis".to_string(),
                "pending".to_string(),
                None,
                None,
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn propose_hypothesis_change_rejects_empty_rationale(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        let server = build_server(db.clone());
        let hypothesis_id = seed_hypothesis(
            &db,
            Some(strategy_id),
            "own hypothesis",
            "body",
            "unverified",
        )
        .await;

        let err = server
            .propose_hypothesis_change_inner(
                strategy_id,
                ProposeHypothesisChangeParams {
                    hypothesis_id,
                    proposed_title: Some("new title".into()),
                    proposed_body: None,
                    proposed_status: None,
                    rationale: "   ".into(),
                },
            )
            .await
            .expect_err("empty rationale expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn propose_hypothesis_change_rejects_no_proposed_fields(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        let server = build_server(db.clone());
        let hypothesis_id = seed_hypothesis(
            &db,
            Some(strategy_id),
            "own hypothesis",
            "body",
            "unverified",
        )
        .await;

        let err = server
            .propose_hypothesis_change_inner(
                strategy_id,
                ProposeHypothesisChangeParams {
                    hypothesis_id,
                    proposed_title: None,
                    proposed_body: None,
                    proposed_status: None,
                    rationale: "just a rationale".into(),
                },
            )
            .await
            .expect_err("no proposed fields expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn propose_hypothesis_change_rejects_invalid_status(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        let server = build_server(db.clone());
        let hypothesis_id = seed_hypothesis(
            &db,
            Some(strategy_id),
            "own hypothesis",
            "body",
            "unverified",
        )
        .await;

        let err = server
            .propose_hypothesis_change_inner(
                strategy_id,
                ProposeHypothesisChangeParams {
                    hypothesis_id,
                    proposed_title: None,
                    proposed_body: None,
                    proposed_status: Some("bogus".into()),
                    rationale: "just a rationale".into(),
                },
            )
            .await
            .expect_err("invalid status expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn propose_hypothesis_change_rejects_other_strategy_hypothesis(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());
        let hypothesis_id = seed_hypothesis(
            &db,
            Some(strategy_b),
            "b's hypothesis",
            "body",
            "unverified",
        )
        .await;

        let err = server
            .propose_hypothesis_change_inner(
                strategy_a,
                ProposeHypothesisChangeParams {
                    hypothesis_id,
                    proposed_title: Some("hijacked title".into()),
                    proposed_body: None,
                    proposed_status: None,
                    rationale: "just a rationale".into(),
                },
            )
            .await
            .expect_err("cross-strategy hypothesis expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}
