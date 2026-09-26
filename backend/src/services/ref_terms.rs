//! ref_term (参照型の別名) の読み出しと、id 優先 -> 別名フォールバックの解決ロジック。
//!
//! 別名の追加・削除は `mcp/strategy/ref_terms.rs` の MCP tool から行う。
//! 内部リンク解決に利用する。

use std::collections::HashMap;

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::entities::{indicator, ref_term, sector, stock, theme};
use crate::error::AppError;
use crate::models::RefResolution;
use crate::text_normalize::normalize;

/// 指定 ref_kind に属する term の集合をまとめて別名解決する。ref_kind ごとに
/// 1 クエリで全別名をロードし、Rust 側で正規化 (NFKC -> lowercase) して比較する。
/// 戻り値は入力の term (正規化前) -> 一致した ref_id 一覧。
pub async fn resolve_many_by_term(
    db: &impl sea_orm::ConnectionTrait,
    ref_kind: &str,
    terms: &[String],
) -> Result<HashMap<String, Vec<String>>, AppError> {
    let rows = ref_term::Entity::find()
        .filter(ref_term::Column::RefKind.eq(ref_kind))
        .all(db)
        .await?;
    let mut by_normalized: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        // 表記違いの別名 (例: "DemoTerm" / "DEMOTERM") が同じ ref_id に複数登録されて
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

/// ref_kind ごとに master テーブルから id -> name を引く
async fn fetch_master_names(
    db: &impl sea_orm::ConnectionTrait,
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
    db: &impl sea_orm::ConnectionTrait,
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
    use std::collections::{BTreeMap, HashMap};

    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};

    use sqlx::PgPool;

    use crate::entities::{indicator, ref_term, stock};
    use crate::models::RefResolution;
    use crate::testing::create_test_db;

    use super::{resolve_many_by_term, resolve_refs};

    fn sort_matches(matches: HashMap<String, Vec<String>>) -> BTreeMap<String, Vec<String>> {
        matches
            .into_iter()
            .map(|(term, mut ref_ids)| {
                ref_ids.sort();
                (term, ref_ids)
            })
            .collect()
    }

    async fn seed_term(
        db: &impl sea_orm::ConnectionTrait,
        ref_kind: &str,
        ref_id: &str,
        term: &str,
    ) {
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

    async fn seed_stock(db: &impl sea_orm::ConnectionTrait, id: &str, name: &str) {
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

    async fn seed_indicator(db: &impl sea_orm::ConnectionTrait, id: &str, name: &str) {
        indicator::ActiveModel {
            id: Set(id.into()),
            name: Set(name.into()),
            kind: Set("fx".into()),
        }
        .insert(db)
        .await
        .expect("seed indicator");
    }

    #[backend_test_macros::database_test]
    async fn resolve_many_by_term_returns_multiple_candidates(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_term(&db, "stock", "SAMPLE-STOCK-A", "サンプル語").await;
        seed_term(&db, "stock", "SAMPLE-STOCK-B", "サンプル語").await;
        seed_term(&db, "indicator", "SAMPLE-INDEX", "サンプル語").await;

        let matches = resolve_many_by_term(&db, "stock", &["サンプル語".to_string()])
            .await
            .expect("resolve_many_by_term");

        assert_eq!(
            sort_matches(matches),
            BTreeMap::from([(
                "サンプル語".to_string(),
                vec!["SAMPLE-STOCK-A".to_string(), "SAMPLE-STOCK-B".to_string()],
            )]),
        );
    }

    #[backend_test_macros::database_test]
    async fn resolve_many_by_term_returns_empty_when_no_match(pool: PgPool) {
        let db = create_test_db(pool).await;

        let matches = resolve_many_by_term(&db, "stock", &["存在しない".to_string()])
            .await
            .expect("resolve_many_by_term");

        assert_eq!(
            sort_matches(matches),
            BTreeMap::from([("存在しない".to_string(), Vec::<String>::new())]),
        );
    }

    #[backend_test_macros::database_test]
    async fn resolve_many_by_term_matches_normalized_variants(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_term(&db, "indicator", "SAMPLE-INDICATOR", "DemoKey").await;

        let inputs = ["ＤＥＭＯＫＥＹ".to_string(), "demokey".to_string()];
        let matches = resolve_many_by_term(&db, "indicator", &inputs)
            .await
            .expect("resolve_many_by_term");

        assert_eq!(
            sort_matches(matches),
            BTreeMap::from([
                (
                    "ＤＥＭＯＫＥＹ".to_string(),
                    vec!["SAMPLE-INDICATOR".to_string()],
                ),
                ("demokey".to_string(), vec!["SAMPLE-INDICATOR".to_string()]),
            ]),
        );
    }

    #[backend_test_macros::database_test]
    async fn resolve_many_by_term_dedups_candidates_from_normalized_variant_aliases_on_same_ref_id(
        pool: PgPool,
    ) {
        let db = create_test_db(pool).await;
        // 表記違いの別名 (大文字/小文字) が同じ ref_id に 2 件登録されていても、
        // 候補は 1 件に集約される
        seed_term(&db, "stock", "SAMPLE-STOCK", "DemoTerm").await;
        seed_term(&db, "stock", "SAMPLE-STOCK", "DEMOTERM").await;

        let matches = resolve_many_by_term(&db, "stock", &["demoterm".to_string()])
            .await
            .expect("resolve_many_by_term");

        assert_eq!(
            sort_matches(matches),
            BTreeMap::from([("demoterm".to_string(), vec!["SAMPLE-STOCK".to_string()])]),
        );
    }

    #[backend_test_macros::database_test]
    async fn resolve_refs_resolves_exact_id_alias_and_unresolved_in_input_order(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_stock(&db, "SAMPLE-STOCK", "サンプル銘柄").await;
        seed_indicator(&db, "SAMPLE-INDICATOR", "サンプル指標").await;
        seed_term(&db, "indicator", "SAMPLE-INDICATOR", "ＳＡＭＰＬＥＫＥＹ").await;

        let result = resolve_refs(
            &db,
            &[
                ("stock".into(), "SAMPLE-STOCK".into()),
                ("indicator".into(), "samplekey".into()),
                ("stock".into(), "UNKNOWN-STOCK".into()),
            ],
        )
        .await
        .expect("resolve_refs");

        assert_eq!(
            result,
            vec![
                RefResolution {
                    kind: "stock".into(),
                    id: "SAMPLE-STOCK".into(),
                    name: Some("サンプル銘柄".into()),
                },
                RefResolution {
                    kind: "indicator".into(),
                    id: "SAMPLE-INDICATOR".into(),
                    name: Some("サンプル指標".into()),
                },
                RefResolution {
                    kind: "stock".into(),
                    id: "UNKNOWN-STOCK".into(),
                    name: None,
                },
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn resolve_refs_leaves_ambiguous_alias_unresolved(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed_stock(&db, "SAMPLE-STOCK-A", "サンプル銘柄 A").await;
        seed_stock(&db, "SAMPLE-STOCK-B", "サンプル銘柄 B").await;
        seed_term(&db, "stock", "SAMPLE-STOCK-A", "サンプル語").await;
        seed_term(&db, "stock", "SAMPLE-STOCK-B", "サンプル語").await;

        let result = resolve_refs(&db, &[("stock".into(), "サンプル語".into())])
            .await
            .expect("resolve_refs");

        assert_eq!(
            result,
            vec![RefResolution {
                kind: "stock".into(),
                id: "サンプル語".into(),
                name: None,
            }],
        );
    }

    #[backend_test_macros::database_test]
    async fn resolve_refs_leaves_dangling_alias_unresolved(pool: PgPool) {
        let db = create_test_db(pool).await;
        // master に存在しない ref_id を指す dangling な別名
        seed_term(&db, "stock", "MISSING-STOCK", "サンプル語").await;

        let result = resolve_refs(&db, &[("stock".into(), "サンプル語".into())])
            .await
            .expect("resolve_refs");

        assert_eq!(
            result,
            vec![RefResolution {
                kind: "stock".into(),
                id: "サンプル語".into(),
                name: None,
            }],
        );
    }
}
