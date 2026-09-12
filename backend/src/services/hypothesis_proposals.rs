//! 仮説への変更提案 (hypothesis_proposal) の承認/却下ロジック。
//!
//! 戦略エージェントは仮説本体を直接書き換えられず、この提案を経由してのみ変更できる。
//! HTTP handler (`backend/src/handlers/hypothesis_proposals.rs`) から呼ばれる。

use chrono::Utc;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, EntityTrait, IntoActiveModel, TransactionTrait,
};
use uuid::Uuid;

use crate::entities::{hypothesis, hypothesis_proposal};
use crate::error::AppError;

pub const PROPOSAL_STATUSES: [&str; 3] = ["pending", "approved", "rejected"];

async fn find_proposal_or_404(
    db: &DatabaseConnection,
    proposal_id: Uuid,
) -> Result<hypothesis_proposal::Model, AppError> {
    hypothesis_proposal::Entity::find_by_id(proposal_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("hypothesis_proposal {proposal_id} not found")))
}

async fn find_hypothesis_or_404<C: ConnectionTrait>(
    db: &C,
    hypothesis_id: Uuid,
) -> Result<hypothesis::Model, AppError> {
    hypothesis::Entity::find_by_id(hypothesis_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("hypothesis {hypothesis_id} not found")))
}

/// 提案を承認する。`pending` の場合のみ、指定されたフィールドだけを仮説本体に反映する。
/// `approved` は再適用せず現在値を返す (冪等)。`rejected` からは遷移できない。
pub async fn approve_proposal(
    db: &DatabaseConnection,
    proposal_id: Uuid,
    review_note: Option<String>,
) -> Result<(hypothesis_proposal::Model, hypothesis::Model), AppError> {
    let proposal = find_proposal_or_404(db, proposal_id).await?;

    match proposal.status.as_str() {
        "rejected" => Err(AppError::Conflict("proposal already rejected".into())),
        "approved" => {
            let current_hypothesis = find_hypothesis_or_404(db, proposal.hypothesis_id).await?;
            Ok((proposal, current_hypothesis))
        }
        _ => {
            let txn = db.begin().await?;

            let current_hypothesis = find_hypothesis_or_404(&txn, proposal.hypothesis_id).await?;
            let mut active_hypothesis = current_hypothesis.into_active_model();
            if let Some(title) = proposal.proposed_title.clone() {
                active_hypothesis.title = Set(title);
            }
            if let Some(body) = proposal.proposed_body.clone() {
                active_hypothesis.body = Set(body);
            }
            if let Some(status) = proposal.proposed_status.clone() {
                active_hypothesis.status = Set(status);
            }
            active_hypothesis.updated_at = Set(Utc::now().fixed_offset());
            let updated_hypothesis = active_hypothesis.update(&txn).await?;

            let mut active_proposal = proposal.into_active_model();
            active_proposal.status = Set("approved".to_string());
            active_proposal.reviewed_at = Set(Some(Utc::now().fixed_offset()));
            active_proposal.review_note = Set(review_note);
            let updated_proposal = active_proposal.update(&txn).await?;

            txn.commit().await?;
            Ok((updated_proposal, updated_hypothesis))
        }
    }
}

/// 提案を却下する。hypothesis 本体には触れない。`rejected` は再適用せず現在値を返す (冪等)。
/// `approved` からは遷移できない。
pub async fn reject_proposal(
    db: &DatabaseConnection,
    proposal_id: Uuid,
    review_note: Option<String>,
) -> Result<hypothesis_proposal::Model, AppError> {
    let proposal = find_proposal_or_404(db, proposal_id).await?;

    match proposal.status.as_str() {
        "approved" => Err(AppError::Conflict("proposal already approved".into())),
        "rejected" => Ok(proposal),
        _ => {
            let mut active = proposal.into_active_model();
            active.status = Set("rejected".to_string());
            active.reviewed_at = Set(Some(Utc::now().fixed_offset()));
            active.review_note = Set(review_note);
            Ok(active.update(db).await?)
        }
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::ActiveValue::NotSet;
    use sqlx::PgPool;

    use super::*;
    use crate::testing::{create_test_db, insert_test_strategy};

    /// 動的な timestamp を固定値に潰してから比較するための epoch。
    fn fixed() -> chrono::DateTime<chrono::FixedOffset> {
        chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset()
    }

    async fn seed_hypothesis(
        db: &DatabaseConnection,
        strategy_id: Uuid,
        title: &str,
        body: &str,
        status: &str,
    ) -> Uuid {
        let id = Uuid::new_v4();
        hypothesis::ActiveModel {
            hypothesis_id: Set(id),
            strategy_id: Set(Some(strategy_id)),
            title: Set(title.into()),
            body: Set(body.into()),
            status: Set(status.into()),
            related_note_ids: Set(vec![]),
            related_interest_ids: Set(vec![]),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert test hypothesis");
        id
    }

    async fn seed_proposal(
        db: &DatabaseConnection,
        hypothesis_id: Uuid,
        proposed_title: Option<&str>,
        proposed_body: Option<&str>,
        proposed_status: Option<&str>,
        rationale: &str,
        status: &str,
    ) -> Uuid {
        let id = Uuid::new_v4();
        hypothesis_proposal::ActiveModel {
            id: Set(id),
            hypothesis_id: Set(hypothesis_id),
            proposed_title: Set(proposed_title.map(str::to_string)),
            proposed_body: Set(proposed_body.map(str::to_string)),
            proposed_status: Set(proposed_status.map(str::to_string)),
            rationale: Set(rationale.into()),
            status: Set(status.into()),
            review_note: Set(None),
            created_at: NotSet,
            reviewed_at: Set(None),
        }
        .insert(db)
        .await
        .expect("insert test hypothesis_proposal");
        id
    }

    fn normalize_hypothesis(mut m: hypothesis::Model) -> hypothesis::Model {
        m.created_at = fixed();
        m.updated_at = fixed();
        m
    }

    fn normalize_proposal(mut m: hypothesis_proposal::Model) -> hypothesis_proposal::Model {
        m.created_at = fixed();
        m.reviewed_at = m.reviewed_at.map(|_| fixed());
        m
    }

    // `sqlx::test` は `#[rstest]` と共存できないため (`src/mcp/strategy/eval_indicator.rs` 参照)、
    // ケースは 1 つの sqlx::test 関数内でループ列挙する。

    #[sqlx::test(migrations = false)]
    async fn approve_applies_only_specified_fields(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let hid = seed_hypothesis(&db, strategy_id, "元タイトル", "元本文", "unverified").await;
        let pid = seed_proposal(
            &db,
            hid,
            Some("新タイトル"),
            None,
            Some("supported"),
            "根拠",
            "pending",
        )
        .await;

        let (proposal, updated_hypothesis) = approve_proposal(&db, pid, Some("良さそう".into()))
            .await
            .expect("approve");

        assert_eq!(
            (
                normalize_proposal(proposal),
                normalize_hypothesis(updated_hypothesis)
            ),
            (
                hypothesis_proposal::Model {
                    id: pid,
                    hypothesis_id: hid,
                    proposed_title: Some("新タイトル".into()),
                    proposed_body: None,
                    proposed_status: Some("supported".into()),
                    rationale: "根拠".into(),
                    status: "approved".into(),
                    review_note: Some("良さそう".into()),
                    created_at: fixed(),
                    reviewed_at: Some(fixed()),
                },
                hypothesis::Model {
                    hypothesis_id: hid,
                    strategy_id: Some(strategy_id),
                    title: "新タイトル".into(),
                    body: "元本文".into(),
                    status: "supported".into(),
                    related_note_ids: vec![],
                    related_interest_ids: vec![],
                    created_at: fixed(),
                    updated_at: fixed(),
                },
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn approve_is_idempotent_and_does_not_reapply(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let hid = seed_hypothesis(&db, strategy_id, "元タイトル", "元本文", "unverified").await;
        let pid = seed_proposal(&db, hid, Some("新タイトル"), None, None, "根拠", "pending").await;

        let (first_proposal, _) = approve_proposal(&db, pid, Some("最初の note".into()))
            .await
            .expect("first approve");

        // 承認後に人間が直接タイトルを編集しても、2 回目の approve は再適用しないため
        // この変更は保持される。
        let mut active = hypothesis::Entity::find_by_id(hid)
            .one(&db)
            .await
            .expect("query")
            .expect("hypothesis exists")
            .into_active_model();
        active.title = Set("人間による再編集".into());
        active.updated_at = Set(fixed());
        let hypothesis_after_human_edit = active.update(&db).await.expect("update hypothesis");

        let (second_proposal, second_hypothesis) =
            approve_proposal(&db, pid, Some("2 回目の note".into()))
                .await
                .expect("second approve");

        assert_eq!(
            (normalize_proposal(second_proposal), second_hypothesis),
            (
                normalize_proposal(first_proposal),
                hypothesis_after_human_edit
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn reject_then_approve_is_conflict(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let hid = seed_hypothesis(&db, strategy_id, "t", "b", "unverified").await;
        let pid = seed_proposal(&db, hid, Some("new"), None, None, "根拠", "pending").await;

        reject_proposal(&db, pid, None).await.expect("reject");

        let err = approve_proposal(&db, pid, None)
            .await
            .expect_err("approve after reject must fail");
        assert_eq!(err.to_string(), "conflict: proposal already rejected");
    }

    #[sqlx::test(migrations = false)]
    async fn reject_is_idempotent(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let hid = seed_hypothesis(&db, strategy_id, "t", "b", "unverified").await;
        let pid = seed_proposal(&db, hid, Some("new"), None, None, "根拠", "pending").await;

        let first = reject_proposal(&db, pid, Some("最初の note".into()))
            .await
            .expect("first reject");
        let second = reject_proposal(&db, pid, Some("2 回目の note".into()))
            .await
            .expect("second reject");

        assert_eq!(normalize_proposal(second), normalize_proposal(first));
    }

    #[sqlx::test(migrations = false)]
    async fn approve_then_reject_is_conflict(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let hid = seed_hypothesis(&db, strategy_id, "t", "b", "unverified").await;
        let pid = seed_proposal(&db, hid, Some("new"), None, None, "根拠", "pending").await;

        approve_proposal(&db, pid, None).await.expect("approve");

        let err = reject_proposal(&db, pid, None)
            .await
            .expect_err("reject after approve must fail");
        assert_eq!(err.to_string(), "conflict: proposal already approved");
    }

    #[sqlx::test(migrations = false)]
    async fn approve_and_reject_return_not_found_for_unknown_id(pool: PgPool) {
        let db = create_test_db(pool).await;
        let unknown = Uuid::new_v4();

        for label_and_result in [
            (
                "approve",
                approve_proposal(&db, unknown, None)
                    .await
                    .map(|_| ())
                    .expect_err("approve of unknown id must fail"),
            ),
            (
                "reject",
                reject_proposal(&db, unknown, None)
                    .await
                    .map(|_| ())
                    .expect_err("reject of unknown id must fail"),
            ),
        ] {
            let (label, err) = label_and_result;
            assert_eq!(
                err.to_string(),
                format!("not found: hypothesis_proposal {unknown} not found"),
                "case {label} message mismatch",
            );
        }
    }
}
