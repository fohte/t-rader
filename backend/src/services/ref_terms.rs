//! ref_term (参照型の別名) の読み出しと、id 優先 -> 別名フォールバックの解決ロジック。
//!
//! 別名の追加・削除は `mcp/strategy/ref_terms.rs` の MCP tool から行う。
//! 内部リンク解決に利用する。

use std::collections::HashMap;

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::entities::{indicator, ref_term, sector, stock, theme};
use crate::error::AppError;
use crate::models::RefResolution;
use crate::text_normalize::normalize;

async fn exists_in_master(
    db: &DatabaseConnection,
    ref_kind: &str,
    ref_id: &str,
) -> Result<bool, AppError> {
    Ok(match ref_kind {
        "stock" => stock::Entity::find_by_id(ref_id).one(db).await?.is_some(),
        "indicator" => indicator::Entity::find_by_id(ref_id)
            .one(db)
            .await?
            .is_some(),
        "sector" => sector::Entity::find_by_id(ref_id).one(db).await?.is_some(),
        "theme" => theme::Entity::find_by_id(ref_id).one(db).await?.is_some(),
        _ => false,
    })
}

/// 指定 ref_kind に属する term の集合をまとめて別名解決する。ref_kind ごとに
/// 1 クエリで全別名をロードし、Rust 側で正規化 (NFKC -> lowercase) して比較する。
/// 戻り値は入力の term (正規化前) -> 一致した ref_id 一覧。
pub async fn resolve_many_by_term(
    db: &DatabaseConnection,
    ref_kind: &str,
    terms: &[String],
) -> Result<HashMap<String, Vec<String>>, AppError> {
    let rows = ref_term::Entity::find()
        .filter(ref_term::Column::RefKind.eq(ref_kind))
        .all(db)
        .await?;
    let mut by_normalized: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        // 表記違いの別名 (例: "Toyota" / "TOYOTA") が同じ ref_id に複数登録されて
        // いても、正規化後は 1 件の候補として扱う。dedup しないと同じ ref_id が
        // 2 件以上の「候補」に見えてしまい、本来一意に解決できるはずの別名が
        // 誤って曖昧判定される
        let candidates = by_normalized.entry(normalize(&row.term)).or_default();
        if !candidates.contains(&row.ref_id) {
            candidates.push(row.ref_id);
        }
    }
    Ok(terms
        .iter()
        .map(|term| {
            let candidates = by_normalized
                .get(&normalize(term))
                .cloned()
                .unwrap_or_default();
            (term.clone(), candidates)
        })
        .collect())
}

/// 別名から同じ ref_kind 内の正規の ref_id 候補を引く。2 件以上ヒットした場合、
/// どれが正しいかはコード側で判断しない。呼び出し側で未解決として扱うこと。
pub async fn resolve_by_term(
    db: &DatabaseConnection,
    ref_kind: &str,
    term: &str,
) -> Result<Vec<String>, AppError> {
    let terms = vec![term.to_string()];
    let mut result = resolve_many_by_term(db, ref_kind, &terms).await?;
    Ok(result.remove(term).unwrap_or_default())
}

/// id の完全一致を優先し、当たらなければ別名で解決する。別名が 1 件だけ当たれば
/// 正規の ref_id を返す。id にも当たらず、別名が 0 件 or 2 件以上のときは None。
/// ref_term は master 存在チェックなしで登録できるため、別名の解決先が master に
/// 存在しない場合も None を返す。
pub async fn resolve_ref_id(
    db: &DatabaseConnection,
    ref_kind: &str,
    ref_id: &str,
) -> Result<Option<String>, AppError> {
    if exists_in_master(db, ref_kind, ref_id).await? {
        return Ok(Some(ref_id.to_string()));
    }
    let candidates = resolve_by_term(db, ref_kind, ref_id).await?;
    let [only] = candidates.as_slice() else {
        return Ok(None);
    };
    if exists_in_master(db, ref_kind, only).await? {
        Ok(Some(only.clone()))
    } else {
        Ok(None)
    }
}

/// ref_kind ごとに master テーブルから id -> name を引く
async fn fetch_master_names(
    db: &DatabaseConnection,
    ids_by_kind: &HashMap<String, Vec<String>>,
) -> Result<HashMap<(String, String), String>, AppError> {
    let mut names = HashMap::new();
    if let Some(ids) = ids_by_kind.get("stock") {
        for m in stock::Entity::find()
            .filter(stock::Column::Id.is_in(ids.clone()))
            .all(db)
            .await?
        {
            names.insert(("stock".to_string(), m.id), m.name);
        }
    }
    if let Some(ids) = ids_by_kind.get("indicator") {
        for m in indicator::Entity::find()
            .filter(indicator::Column::Id.is_in(ids.clone()))
            .all(db)
            .await?
        {
            names.insert(("indicator".to_string(), m.id), m.name);
        }
    }
    if let Some(ids) = ids_by_kind.get("sector") {
        for m in sector::Entity::find()
            .filter(sector::Column::Id.is_in(ids.clone()))
            .all(db)
            .await?
        {
            names.insert(("sector".to_string(), m.id), m.name);
        }
    }
    if let Some(ids) = ids_by_kind.get("theme") {
        for m in theme::Entity::find()
            .filter(theme::Column::Id.is_in(ids.clone()))
            .all(db)
            .await?
        {
            names.insert(("theme".to_string(), m.id), m.name);
        }
    }
    Ok(names)
}

fn group_by_kind(requested: &[(String, String)]) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for (kind, id) in requested {
        out.entry(kind.clone()).or_default().push(id.clone());
    }
    out
}

/// `[[kind:id]]` 参照の一括解決。id の完全一致を別名より優先し、id で解決できなかった
/// ものだけ別名解決を試みる。別名で解決できたときは正規の id と name を返す。
/// どちらにも当たらなければ name = None、id は入力のまま返す。入力の順序は保つ。
pub async fn resolve_refs(
    db: &DatabaseConnection,
    requested: &[(String, String)],
) -> Result<Vec<RefResolution>, AppError> {
    let mut names = fetch_master_names(db, &group_by_kind(requested)).await?;

    let mut unresolved_by_kind: HashMap<String, Vec<String>> = HashMap::new();
    for (kind, id) in requested {
        if !names.contains_key(&(kind.clone(), id.clone())) {
            unresolved_by_kind
                .entry(kind.clone())
                .or_default()
                .push(id.clone());
        }
    }

    let mut alias_target: HashMap<(String, String), String> = HashMap::new();
    for (kind, ids) in &unresolved_by_kind {
        let resolved = resolve_many_by_term(db, kind, ids).await?;
        for (id, candidates) in resolved {
            if let [only] = candidates.as_slice() {
                alias_target.insert((kind.clone(), id), only.clone());
            }
        }
    }

    let target_ids_by_kind = group_by_kind(
        &alias_target
            .iter()
            .map(|((kind, _), target_id)| (kind.clone(), target_id.clone()))
            .collect::<Vec<_>>(),
    );
    if !target_ids_by_kind.is_empty() {
        let target_names = fetch_master_names(db, &target_ids_by_kind).await?;
        // 別名の解決先が master に存在しない (dangling な別名) 場合は解決扱いにしない
        alias_target.retain(|(kind, _), target_id| {
            target_names.contains_key(&(kind.clone(), target_id.clone()))
        });
        names.extend(target_names);
    }

    Ok(requested
        .iter()
        .map(|(kind, id)| {
            let key = (kind.clone(), id.clone());
            if let Some(name) = names.get(&key) {
                return RefResolution {
                    kind: kind.clone(),
                    id: id.clone(),
                    name: Some(name.clone()),
                };
            }
            if let Some(target_id) = alias_target.get(&key) {
                let name = names.get(&(kind.clone(), target_id.clone())).cloned();
                return RefResolution {
                    kind: kind.clone(),
                    id: target_id.clone(),
                    name,
                };
            }
            RefResolution {
                kind: kind.clone(),
                id: id.clone(),
                name: None,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::DatabaseConnection;
    use sqlx::PgPool;

    use crate::entities::{indicator, ref_term, stock};
    use crate::models::RefResolution;
    use crate::testing::create_test_db;

    use super::{resolve_by_term, resolve_ref_id, resolve_refs};

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

    async fn seed_stock(db: &DatabaseConnection, id: &str, name: &str) {
        stock::ActiveModel {
            id: Set(id.into()),
            name: Set(name.into()),
            market: Set(None),
            sector_id: Set(None),
            product_category: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("seed stock");
    }

    async fn seed_indicator(db: &DatabaseConnection, id: &str, name: &str) {
        indicator::ActiveModel {
            id: Set(id.into()),
            name: Set(name.into()),
            kind: Set("fx".into()),
        }
        .insert(db)
        .await
        .expect("seed indicator");
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

    #[sqlx::test(migrations = false)]
    async fn resolve_by_term_matches_normalized_variants(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_term(&db, "indicator", "USDJPY", "USDJPY").await;

        for (label, input) in [("zenkaku", "ＵＳＤＪＰＹ"), ("lowercase", "usdjpy")] {
            let ref_ids = resolve_by_term(&db, "indicator", input)
                .await
                .unwrap_or_else(|e| panic!("resolve_by_term ({label}): {e}"));
            assert_eq!(ref_ids, vec!["USDJPY".to_string()], "case: {label}");
        }
    }

    #[sqlx::test(migrations = false)]
    async fn resolve_by_term_dedups_candidates_from_normalized_variant_aliases_on_same_ref_id(
        pool: PgPool,
    ) {
        let db = create_test_db(pool).await;
        // 表記違いの別名 (大文字/小文字) が同じ ref_id に 2 件登録されていても、
        // 候補は 1 件に集約される
        seed_term(&db, "stock", "7203", "Toyota").await;
        seed_term(&db, "stock", "7203", "TOYOTA").await;

        let ref_ids = resolve_by_term(&db, "stock", "toyota")
            .await
            .expect("resolve_by_term");

        assert_eq!(ref_ids, vec!["7203".to_string()]);
    }

    #[sqlx::test(migrations = false)]
    async fn resolve_ref_id_prefers_exact_master_id_over_alias(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_stock(&db, "7203", "トヨタ自動車").await;
        // 7203 という語を別の銘柄の別名として登録していても、id の完全一致が優先される
        seed_term(&db, "stock", "9984", "7203").await;

        let resolved = resolve_ref_id(&db, "stock", "7203")
            .await
            .expect("resolve_ref_id");

        assert_eq!(resolved, Some("7203".to_string()));
    }

    #[sqlx::test(migrations = false)]
    async fn resolve_ref_id_resolves_unique_alias_when_id_not_in_master(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_stock(&db, "7203", "トヨタ自動車").await;
        seed_term(&db, "stock", "7203", "Ｔｏｙｏｔａ").await;

        let resolved = resolve_ref_id(&db, "stock", "toyota")
            .await
            .expect("resolve_ref_id");

        assert_eq!(resolved, Some("7203".to_string()));
    }

    #[sqlx::test(migrations = false)]
    async fn resolve_ref_id_returns_none_when_alias_is_ambiguous(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_stock(&db, "7203", "トヨタ自動車").await;
        seed_stock(&db, "9984", "ソフトバンクグループ").await;
        seed_term(&db, "stock", "7203", "トヨタ").await;
        seed_term(&db, "stock", "9984", "トヨタ").await;

        let resolved = resolve_ref_id(&db, "stock", "トヨタ")
            .await
            .expect("resolve_ref_id");

        assert_eq!(resolved, None);
    }

    #[sqlx::test(migrations = false)]
    async fn resolve_ref_id_returns_none_when_alias_target_is_not_in_master(pool: PgPool) {
        let db = create_test_db(pool).await;
        // master に存在しない ref_id (9999) を指す dangling な別名
        seed_term(&db, "stock", "9999", "トヨタ").await;

        let resolved = resolve_ref_id(&db, "stock", "トヨタ")
            .await
            .expect("resolve_ref_id");

        assert_eq!(resolved, None);
    }

    #[sqlx::test(migrations = false)]
    async fn resolve_ref_id_returns_none_when_neither_id_nor_alias_match(pool: PgPool) {
        let db = create_test_db(pool).await;

        let resolved = resolve_ref_id(&db, "stock", "存在しない")
            .await
            .expect("resolve_ref_id");

        assert_eq!(resolved, None);
    }

    #[sqlx::test(migrations = false)]
    async fn resolve_refs_resolves_exact_id_alias_and_unresolved_in_input_order(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_stock(&db, "7203", "トヨタ自動車").await;
        seed_indicator(&db, "USDJPY", "ドル円").await;
        seed_term(&db, "indicator", "USDJPY", "ＵＳＤＪＰＹ").await;

        let result = resolve_refs(
            &db,
            &[
                ("stock".into(), "7203".into()),
                ("indicator".into(), "ＵＳＤＪＰＹ".into()),
                ("stock".into(), "9999".into()),
            ],
        )
        .await
        .expect("resolve_refs");

        assert_eq!(
            result,
            vec![
                RefResolution {
                    kind: "stock".into(),
                    id: "7203".into(),
                    name: Some("トヨタ自動車".into()),
                },
                RefResolution {
                    kind: "indicator".into(),
                    id: "USDJPY".into(),
                    name: Some("ドル円".into()),
                },
                RefResolution {
                    kind: "stock".into(),
                    id: "9999".into(),
                    name: None,
                },
            ],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn resolve_refs_leaves_ambiguous_alias_unresolved(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_stock(&db, "7203", "トヨタ自動車").await;
        seed_stock(&db, "9984", "ソフトバンクグループ").await;
        seed_term(&db, "stock", "7203", "トヨタ").await;
        seed_term(&db, "stock", "9984", "トヨタ").await;

        let result = resolve_refs(&db, &[("stock".into(), "トヨタ".into())])
            .await
            .expect("resolve_refs");

        assert_eq!(
            result,
            vec![RefResolution {
                kind: "stock".into(),
                id: "トヨタ".into(),
                name: None,
            }],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn resolve_refs_leaves_dangling_alias_unresolved(pool: PgPool) {
        let db = create_test_db(pool).await;
        // master に存在しない ref_id (9999) を指す dangling な別名
        seed_term(&db, "stock", "9999", "トヨタ").await;

        let result = resolve_refs(&db, &[("stock".into(), "トヨタ".into())])
            .await
            .expect("resolve_refs");

        assert_eq!(
            result,
            vec![RefResolution {
                kind: "stock".into(),
                id: "トヨタ".into(),
                name: None,
            }],
        );
    }
}
