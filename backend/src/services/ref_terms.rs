//! ref_term (参照型の別名) の読み出し。
//!
//! 別名の追加・削除は `mcp/strategy/ref_terms.rs` の MCP tool から行う。ここは
//! 正規化を持たない (ref_kind, ref_id, term) の集合をそのまま読み出すだけの層で、
//! ニュースの語マッチ (`services/news.rs`) から利用する。内部リンク解決からの
//! 利用は後続の別 PR で追加される。

use sea_orm::{ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter};

use crate::entities::ref_term;
use crate::error::AppError;

/// 指定した (ref_kind, ref_id) の集合に登録された別名をまとめて返す。
pub async fn load_terms(
    db: &DatabaseConnection,
    refs: &[(String, String)],
) -> Result<Vec<ref_term::Model>, AppError> {
    if refs.is_empty() {
        return Ok(vec![]);
    }
    let condition = refs
        .iter()
        .fold(Condition::any(), |cond, (ref_kind, ref_id)| {
            cond.add(
                Condition::all()
                    .add(ref_term::Column::RefKind.eq(ref_kind.as_str()))
                    .add(ref_term::Column::RefId.eq(ref_id.as_str())),
            )
        });
    let terms = ref_term::Entity::find().filter(condition).all(db).await?;
    Ok(terms)
}

/// 別名から同じ ref_kind 内の正規の ref_id 候補を引く。2 件以上ヒットした場合、
/// どれが正しいかはコード側で判断しない。呼び出し側で未解決として扱うこと。
pub async fn resolve_by_term(
    db: &DatabaseConnection,
    ref_kind: &str,
    term: &str,
) -> Result<Vec<String>, AppError> {
    let rows = ref_term::Entity::find()
        .filter(ref_term::Column::RefKind.eq(ref_kind))
        .filter(ref_term::Column::Term.eq(term))
        .all(db)
        .await?;
    Ok(rows.into_iter().map(|row| row.ref_id).collect())
}

#[cfg(test)]
mod tests {
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::DatabaseConnection;
    use sqlx::PgPool;

    use crate::entities::ref_term;
    use crate::testing::create_test_db;

    use super::{load_terms, resolve_by_term};

    async fn seed_term(db: &DatabaseConnection, ref_kind: &str, ref_id: &str, term: &str) {
        ref_term::ActiveModel {
            ref_kind: Set(ref_kind.into()),
            ref_id: Set(ref_id.into()),
            term: Set(term.into()),
            origin: Set("human".into()),
            created_at: NotSet,
        }
        .insert(db)
        .await
        .expect("seed ref_term");
    }

    #[sqlx::test(migrations = false)]
    async fn load_terms_returns_only_requested_refs(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_term(&db, "stock", "7203", "トヨタ").await;
        seed_term(&db, "stock", "7203", "Toyota").await;
        seed_term(&db, "stock", "9984", "ソフトバンク").await;
        seed_term(&db, "indicator", "USDJPY", "ドル円").await;

        let mut terms = load_terms(&db, &[("stock".into(), "7203".into())])
            .await
            .expect("load_terms");
        terms.sort_by(|a, b| a.term.cmp(&b.term));

        assert_eq!(
            terms.into_iter().map(|t| t.term).collect::<Vec<_>>(),
            vec!["Toyota".to_string(), "トヨタ".to_string()],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn load_terms_returns_empty_for_empty_refs(pool: PgPool) {
        let db = create_test_db(pool).await;

        let terms = load_terms(&db, &[]).await.expect("load_terms");

        assert_eq!(terms, vec![]);
    }

    #[sqlx::test(migrations = false)]
    async fn resolve_by_term_can_return_multiple_candidates(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_term(&db, "stock", "7203", "トヨタ").await;
        seed_term(&db, "stock", "9984", "トヨタ").await;
        seed_term(&db, "indicator", "TOYOTA_IDX", "トヨタ").await;

        let mut ref_ids = resolve_by_term(&db, "stock", "トヨタ")
            .await
            .expect("resolve_by_term");
        ref_ids.sort();

        assert_eq!(ref_ids, vec!["7203".to_string(), "9984".to_string()]);
    }

    #[sqlx::test(migrations = false)]
    async fn resolve_by_term_returns_empty_when_no_match(pool: PgPool) {
        let db = create_test_db(pool).await;

        let ref_ids = resolve_by_term(&db, "stock", "存在しない")
            .await
            .expect("resolve_by_term");

        assert_eq!(ref_ids, Vec::<String>::new());
    }
}
