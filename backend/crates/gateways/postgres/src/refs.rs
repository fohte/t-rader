use std::collections::HashMap;

use async_trait::async_trait;
use core_application::refs::{
    IndicatorRef, RefRepository, RefRepositoryError, RefSearchMatch, RefTerm, SectorRef, StockRef,
    ThemeRef,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, EntityTrait, FromQueryResult, QueryFilter, QueryOrder,
    QuerySelect, Statement,
};

use crate::DatabaseHandle;
use crate::entities::{indicator, ref_term, sector, stock, theme};
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;

const SEARCH_REFS_SQL: &str = r#"
    SELECT 'stock' AS ref_kind, s.id AS ref_id, s.name, s.product_category
        FROM stock s
        WHERE normalize(s.id, NFKC) ILIKE $1
           OR normalize(s.name, NFKC) ILIKE $1
           OR EXISTS (
               SELECT 1 FROM ref_term t
               WHERE t.ref_kind = 'stock' AND t.ref_id = s.id
                 AND normalize(t.term, NFKC) ILIKE $1
           )
    UNION ALL
    SELECT 'indicator', i.id, i.name, NULL
        FROM indicator i
        WHERE normalize(i.id, NFKC) ILIKE $1
           OR normalize(i.name, NFKC) ILIKE $1
           OR EXISTS (
               SELECT 1 FROM ref_term t
               WHERE t.ref_kind = 'indicator' AND t.ref_id = i.id
                 AND normalize(t.term, NFKC) ILIKE $1
           )
    UNION ALL
    SELECT 'sector', se.id, se.name, NULL
        FROM sector se
        WHERE normalize(se.id, NFKC) ILIKE $1
           OR normalize(se.name, NFKC) ILIKE $1
           OR EXISTS (
               SELECT 1 FROM ref_term t
               WHERE t.ref_kind = 'sector' AND t.ref_id = se.id
                 AND normalize(t.term, NFKC) ILIKE $1
           )
    UNION ALL
    SELECT 'theme', th.id, th.name, NULL
        FROM theme th
        WHERE normalize(th.id, NFKC) ILIKE $1
           OR normalize(th.name, NFKC) ILIKE $1
           OR EXISTS (
               SELECT 1 FROM ref_term t
               WHERE t.ref_kind = 'theme' AND t.ref_id = th.id
                 AND normalize(t.term, NFKC) ILIKE $1
           )
    ORDER BY name, ref_kind
    LIMIT $2
"#;

#[derive(Debug, FromQueryResult)]
struct RefSearchRow {
    ref_kind: String,
    ref_id: String,
    name: String,
    product_category: Option<String>,
}

#[derive(Clone)]
pub struct PostgresRefRepository {
    db: DatabaseHandle,
}

impl PostgresRefRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl RefRepository for PostgresRefRepository {
    async fn list_stocks(&self, query: Option<&str>) -> Result<Vec<StockRef>, RefRepositoryError> {
        let mut find = stock::Entity::find().order_by_asc(stock::Column::Id);
        if let Some(query) = query {
            let pattern = format!("%{query}%");
            find = find.filter(
                Condition::any()
                    .add(stock::Column::Id.like(&pattern))
                    .add(stock::Column::Name.like(&pattern)),
            );
        }
        find.limit(50)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_stock_ref).collect())
            .map_err(repository_error)
    }

    async fn find_stock(&self, id: &str) -> Result<Option<StockRef>, RefRepositoryError> {
        stock::Entity::find_by_id(id.to_string())
            .one(&self.db)
            .await
            .map(|row| row.map(to_stock_ref))
            .map_err(repository_error)
    }

    async fn stock_sectors(
        &self,
        ids: &[String],
    ) -> Result<HashMap<String, Option<String>>, RefRepositoryError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        stock::Entity::find()
            .filter(stock::Column::Id.is_in(ids.to_vec()))
            .all(&self.db)
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|row| (row.id, row.sector_id))
                    .collect()
            })
            .map_err(repository_error)
    }

    async fn list_indicators(
        &self,
        query: Option<&str>,
    ) -> Result<Vec<IndicatorRef>, RefRepositoryError> {
        let mut find = indicator::Entity::find().order_by_asc(indicator::Column::Id);
        if let Some(query) = query {
            let pattern = format!("%{query}%");
            find = find.filter(
                Condition::any()
                    .add(indicator::Column::Id.like(&pattern))
                    .add(indicator::Column::Name.like(&pattern)),
            );
        }
        find.limit(50)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_indicator_ref).collect())
            .map_err(repository_error)
    }

    async fn find_indicator(&self, id: &str) -> Result<Option<IndicatorRef>, RefRepositoryError> {
        indicator::Entity::find_by_id(id.to_string())
            .one(&self.db)
            .await
            .map(|row| row.map(to_indicator_ref))
            .map_err(repository_error)
    }

    async fn list_sectors(
        &self,
        query: Option<&str>,
    ) -> Result<Vec<SectorRef>, RefRepositoryError> {
        let mut find = sector::Entity::find().order_by_asc(sector::Column::Id);
        if let Some(query) = query {
            let pattern = format!("%{query}%");
            find = find.filter(
                Condition::any()
                    .add(sector::Column::Id.like(&pattern))
                    .add(sector::Column::Name.like(&pattern)),
            );
        }
        find.limit(50)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_sector_ref).collect())
            .map_err(repository_error)
    }

    async fn find_sector(&self, id: &str) -> Result<Option<SectorRef>, RefRepositoryError> {
        sector::Entity::find_by_id(id.to_string())
            .one(&self.db)
            .await
            .map(|row| row.map(to_sector_ref))
            .map_err(repository_error)
    }

    async fn list_themes(&self, query: Option<&str>) -> Result<Vec<ThemeRef>, RefRepositoryError> {
        let mut find = theme::Entity::find().order_by_asc(theme::Column::Id);
        if let Some(query) = query {
            let pattern = format!("%{query}%");
            find = find.filter(
                Condition::any()
                    .add(theme::Column::Id.like(&pattern))
                    .add(theme::Column::Name.like(&pattern)),
            );
        }
        find.limit(50)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_theme_ref).collect())
            .map_err(repository_error)
    }

    async fn find_theme(&self, id: &str) -> Result<Option<ThemeRef>, RefRepositoryError> {
        theme::Entity::find_by_id(id.to_string())
            .one(&self.db)
            .await
            .map(|row| row.map(to_theme_ref))
            .map_err(repository_error)
    }

    async fn search_all(
        &self,
        pattern: &str,
        limit: u64,
    ) -> Result<Vec<RefSearchMatch>, RefRepositoryError> {
        let limit = limit.min(i64::MAX as u64) as i64;
        self.db
            .query_all_raw(Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Postgres,
                SEARCH_REFS_SQL,
                [pattern.to_string().into(), limit.into()],
            ))
            .await
            .map_err(repository_error)?
            .iter()
            .map(|row| {
                RefSearchRow::from_query_result(row, "")
                    .map(|row| RefSearchMatch {
                        ref_kind: row.ref_kind,
                        ref_id: row.ref_id,
                        name: row.name,
                        product_category: row.product_category,
                    })
                    .map_err(repository_error)
            })
            .collect()
    }

    async fn stock_names(
        &self,
        ids: &[String],
    ) -> Result<HashMap<String, String>, RefRepositoryError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        stock::Entity::find()
            .filter(stock::Column::Id.is_in(ids.to_vec()))
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(|row| (row.id, row.name)).collect())
            .map_err(repository_error)
    }

    async fn indicator_names(
        &self,
        ids: &[String],
    ) -> Result<HashMap<String, String>, RefRepositoryError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        indicator::Entity::find()
            .filter(indicator::Column::Id.is_in(ids.to_vec()))
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(|row| (row.id, row.name)).collect())
            .map_err(repository_error)
    }

    async fn sector_names(
        &self,
        ids: &[String],
    ) -> Result<HashMap<String, String>, RefRepositoryError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        sector::Entity::find()
            .filter(sector::Column::Id.is_in(ids.to_vec()))
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(|row| (row.id, row.name)).collect())
            .map_err(repository_error)
    }

    async fn theme_names(
        &self,
        ids: &[String],
    ) -> Result<HashMap<String, String>, RefRepositoryError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        theme::Entity::find()
            .filter(theme::Column::Id.is_in(ids.to_vec()))
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(|row| (row.id, row.name)).collect())
            .map_err(repository_error)
    }

    async fn list_terms(&self, ref_kind: &str) -> Result<Vec<RefTerm>, RefRepositoryError> {
        ref_term::Entity::find()
            .filter(ref_term::Column::RefKind.eq(ref_kind))
            .all(&self.db)
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|row| RefTerm {
                        ref_id: row.ref_id,
                        term: row.term,
                    })
                    .collect()
            })
            .map_err(repository_error)
    }

    async fn insert_term(
        &self,
        transaction: &UnitOfWorkTransaction,
        ref_kind: &str,
        ref_id: &str,
        term: &str,
        origin: &str,
    ) -> Result<bool, RefRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = ref_term::ActiveModel {
            ref_kind: Set(ref_kind.to_string()),
            ref_id: Set(ref_id.to_string()),
            term: Set(term.to_string()),
            origin: Set(origin.to_string()),
            created_at: NotSet,
        };
        ref_term::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([
                    ref_term::Column::RefKind,
                    ref_term::Column::RefId,
                    ref_term::Column::Term,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec_without_returning(transaction)
            .await
            .map(|rows_affected| rows_affected > 0)
            .map_err(repository_error)
    }

    async fn delete_term(
        &self,
        transaction: &UnitOfWorkTransaction,
        ref_kind: &str,
        ref_id: &str,
        term: &str,
    ) -> Result<bool, RefRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        ref_term::Entity::delete_many()
            .filter(ref_term::Column::RefKind.eq(ref_kind))
            .filter(ref_term::Column::RefId.eq(ref_id))
            .filter(ref_term::Column::Term.eq(term))
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, RefRepositoryError> {
    postgres_transaction_ref(transaction).ok_or(RefRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> RefRepositoryError {
    RefRepositoryError::Database(persistence_error(error))
}

fn to_stock_ref(model: stock::Model) -> StockRef {
    StockRef {
        id: model.id,
        name: model.name,
        market: model.market,
        sector_id: model.sector_id,
        created_at: model.created_at,
        updated_at: model.updated_at,
        product_category: model.product_category,
    }
}

fn to_indicator_ref(model: indicator::Model) -> IndicatorRef {
    IndicatorRef {
        id: model.id,
        name: model.name,
        kind: model.kind,
    }
}

fn to_sector_ref(model: sector::Model) -> SectorRef {
    SectorRef {
        id: model.id,
        name: model.name,
    }
}

fn to_theme_ref(model: theme::Model) -> ThemeRef {
    ThemeRef {
        id: model.id,
        name: model.name,
        description: model.description,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use core_application::refs::{RefUseCases, ResolvedRef, SharedRefRepository};
    use core_application::unit_of_work::SharedUnitOfWork;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};

    use super::PostgresRefRepository;
    use crate::entities::{indicator, ref_term, sector, stock};
    use crate::unit_of_work::PostgresUnitOfWork;

    fn build_use_cases(db: crate::DatabaseHandle) -> RefUseCases {
        let unit_of_work: SharedUnitOfWork = Arc::new(PostgresUnitOfWork::new(db.clone()));
        let repository: SharedRefRepository = Arc::new(PostgresRefRepository::new(db));
        RefUseCases::new(unit_of_work, repository)
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
        .expect("seed ref term");
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

    async fn seed_stock_with_sector(db: &impl sea_orm::ConnectionTrait, id: &str, sector_id: &str) {
        sector::ActiveModel {
            id: Set(sector_id.into()),
            name: Set(sector_id.into()),
        }
        .insert(db)
        .await
        .expect("seed sector");
        stock::ActiveModel {
            id: Set(id.into()),
            name: Set(id.into()),
            market: Set(None),
            sector_id: Set(Some(sector_id.into())),
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
            kind: Set("sample-kind".into()),
        }
        .insert(db)
        .await
        .expect("seed indicator");
    }

    #[backend_test_macros::database_test]
    async fn stock_sectors_returns_assignments_for_existing_stocks(db: crate::DatabaseHandle) {
        seed_stock_with_sector(&db, "DEMO-STOCK-A", "DEMO-SECTOR").await;
        seed_stock(&db, "DEMO-STOCK-B", "Demo Stock B").await;

        let result = build_use_cases(db)
            .stock_sectors(&[
                "DEMO-STOCK-A".into(),
                "DEMO-STOCK-B".into(),
                "DEMO-STOCK-MISSING".into(),
            ])
            .await
            .expect("find stock sectors");

        assert_eq!(
            result,
            std::collections::HashMap::from([
                ("DEMO-STOCK-A".into(), Some("DEMO-SECTOR".into())),
                ("DEMO-STOCK-B".into(), None),
            ]),
        );
    }

    #[backend_test_macros::database_test]
    async fn resolve_prefers_ids_and_preserves_alias_and_input_order(db: crate::DatabaseHandle) {
        seed_stock(&db, "DEMO-STOCK", "架空銘柄").await;
        seed_indicator(&db, "DEMO-INDICATOR", "架空指標").await;
        seed_term(&db, "indicator", "DEMO-INDICATOR", "ＤＥＭＯＫＥＹ").await;

        let result = build_use_cases(db)
            .resolve(&[
                ("stock".into(), "DEMO-STOCK".into()),
                ("indicator".into(), "demokey".into()),
                ("stock".into(), "UNKNOWN-STOCK".into()),
            ])
            .await
            .expect("resolve refs");

        assert_eq!(
            result,
            vec![
                ResolvedRef {
                    kind: "stock".into(),
                    id: "DEMO-STOCK".into(),
                    name: Some("架空銘柄".into()),
                },
                ResolvedRef {
                    kind: "indicator".into(),
                    id: "DEMO-INDICATOR".into(),
                    name: Some("架空指標".into()),
                },
                ResolvedRef {
                    kind: "stock".into(),
                    id: "UNKNOWN-STOCK".into(),
                    name: None,
                },
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn resolve_keeps_ambiguous_and_dangling_aliases_unresolved(db: crate::DatabaseHandle) {
        seed_stock(&db, "DEMO-STOCK-A", "架空銘柄 A").await;
        seed_stock(&db, "DEMO-STOCK-B", "架空銘柄 B").await;
        seed_term(&db, "stock", "DEMO-STOCK-A", "AMBIGUOUS-ALIAS").await;
        seed_term(&db, "stock", "DEMO-STOCK-B", "AMBIGUOUS-ALIAS").await;
        seed_term(&db, "stock", "MISSING-STOCK", "DANGLING-ALIAS").await;

        let result = build_use_cases(db)
            .resolve(&[
                ("stock".into(), "AMBIGUOUS-ALIAS".into()),
                ("stock".into(), "DANGLING-ALIAS".into()),
            ])
            .await
            .expect("resolve refs");

        assert_eq!(
            result,
            vec![
                ResolvedRef {
                    kind: "stock".into(),
                    id: "AMBIGUOUS-ALIAS".into(),
                    name: None,
                },
                ResolvedRef {
                    kind: "stock".into(),
                    id: "DANGLING-ALIAS".into(),
                    name: None,
                },
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn resolve_deduplicates_normalized_aliases_for_the_same_ref(db: crate::DatabaseHandle) {
        seed_stock(&db, "DEMO-STOCK", "架空銘柄").await;
        seed_term(&db, "stock", "DEMO-STOCK", "DemoAlias").await;
        seed_term(&db, "stock", "DEMO-STOCK", "DEMOALIAS").await;

        let result = build_use_cases(db)
            .resolve(&[("stock".into(), "demoalias".into())])
            .await
            .expect("resolve refs");

        assert_eq!(
            result,
            vec![ResolvedRef {
                kind: "stock".into(),
                id: "DEMO-STOCK".into(),
                name: Some("架空銘柄".into()),
            }],
        );
    }
}
