use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use core_application::refs::{
    IndicatorRef, RefRepository, RefRepositoryError, RefSearchMatch, RefTerm, StockRef,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, EntityTrait, FromQueryResult, QueryFilter, QueryOrder,
    QuerySelect, Statement,
};

use crate::DatabaseHandle;
use crate::entities::{group_axis, indicator, ref_term, stock, stock_group};
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
    SELECT 'group', ga.key || '/' || sg.key, sg.name, NULL
        FROM stock_group sg
        JOIN group_axis ga ON ga.id = sg.axis_id
        WHERE normalize(ga.key || '/' || sg.key, NFKC) ILIKE $1
           OR normalize(sg.name, NFKC) ILIKE $1
           OR EXISTS (
               SELECT 1 FROM ref_term t
               WHERE t.ref_kind = 'group' AND t.ref_id = ga.key || '/' || sg.key
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

#[derive(Debug, FromQueryResult)]
struct StockSectorRow {
    stock_id: String,
    sector_id: Option<String>,
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
        let stocks = find
            .limit(50)
            .all(&self.db)
            .await
            .map_err(repository_error)?;
        let ids = stocks
            .iter()
            .map(|stock| stock.id.clone())
            .collect::<Vec<_>>();
        let sectors = self.stock_sectors(&ids).await?;
        Ok(stocks
            .into_iter()
            .map(|stock| {
                let sector_id = sectors.get(&stock.id).cloned().flatten();
                to_stock_ref(stock, sector_id)
            })
            .collect())
    }

    async fn find_stock(&self, id: &str) -> Result<Option<StockRef>, RefRepositoryError> {
        let stock = stock::Entity::find_by_id(id.to_string())
            .one(&self.db)
            .await
            .map_err(repository_error)?;
        let Some(stock) = stock else {
            return Ok(None);
        };
        let sectors = self.stock_sectors(std::slice::from_ref(&stock.id)).await?;
        let sector_id = sectors.get(&stock.id).cloned().flatten();
        Ok(Some(to_stock_ref(stock, sector_id)))
    }

    async fn stock_sectors(
        &self,
        ids: &[String],
    ) -> Result<HashMap<String, Option<String>>, RefRepositoryError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = (1..=ids.len())
            .map(|index| format!("${index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT s.id AS stock_id, \
                    MIN(sg.key) FILTER (WHERE ga.id IS NOT NULL) AS sector_id \
             FROM stock s \
             LEFT JOIN stock_group_member sgm ON sgm.stock_id = s.id \
             LEFT JOIN stock_group sg ON sg.id = sgm.group_id \
             LEFT JOIN group_axis ga ON ga.id = sg.axis_id AND ga.sync_source = 'jquants' \
             WHERE s.id IN ({placeholders}) \
             GROUP BY s.id"
        );
        let values = ids
            .iter()
            .map(|id| id.clone().into())
            .collect::<Vec<sea_orm::Value>>();
        self.db
            .query_all_raw(Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Postgres,
                sql,
                values,
            ))
            .await
            .map_err(repository_error)?
            .iter()
            .map(|row| {
                StockSectorRow::from_query_result(row, "")
                    .map(|row| (row.stock_id, row.sector_id))
                    .map_err(repository_error)
            })
            .collect()
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

    async fn group_names(
        &self,
        ids: &[String],
    ) -> Result<HashMap<String, String>, RefRepositoryError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let requested_ids = ids.iter().cloned().collect::<HashSet<_>>();
        let group_keys = ids
            .iter()
            .filter_map(|id| id.split_once('/').map(|(_, key)| key.to_string()))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if group_keys.is_empty() {
            return Ok(HashMap::new());
        }
        stock_group::Entity::find()
            .find_also_related(group_axis::Entity)
            .filter(stock_group::Column::Key.is_in(group_keys))
            .all(&self.db)
            .await
            .map_err(repository_error)
            .map(|rows| {
                rows.into_iter()
                    .filter_map(|(group, axis)| {
                        let axis = axis?;
                        let id = format!("{}/{}", axis.key, group.key);
                        requested_ids.contains(&id).then_some((id, group.name))
                    })
                    .collect()
            })
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

fn to_stock_ref(model: stock::Model, sector_id: Option<String>) -> StockRef {
    StockRef {
        id: model.id,
        name: model.name,
        market: model.market,
        sector_id,
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use core_application::refs::{RefUseCases, ResolvedRef, SharedRefRepository};
    use core_application::unit_of_work::SharedUnitOfWork;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};

    use super::PostgresRefRepository;
    use crate::entities::{
        group_axis, indicator, ref_term, stock, stock_group, stock_group_member,
    };
    use crate::unit_of_work::PostgresUnitOfWork;
    use uuid::Uuid;

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

    async fn seed_group(
        db: &impl sea_orm::ConnectionTrait,
        axis_key: &str,
        group_key: &str,
        name: &str,
        sync_source: Option<&str>,
    ) -> (String, Uuid) {
        let axis_id = Uuid::new_v4();
        group_axis::ActiveModel {
            id: Set(axis_id),
            key: Set(axis_key.into()),
            name: Set("Sample Axis".into()),
            description: Set("Sample axis for tests".into()),
            sync_source: Set(sync_source.map(str::to_string)),
        }
        .insert(db)
        .await
        .expect("seed group axis");
        let group_id = Uuid::new_v4();
        stock_group::ActiveModel {
            id: Set(group_id),
            axis_id: Set(axis_id),
            key: Set(group_key.into()),
            name: Set(name.into()),
            description: Set(None),
        }
        .insert(db)
        .await
        .expect("seed stock group");
        (format!("{axis_key}/{group_key}"), group_id)
    }

    async fn seed_membership(db: &impl sea_orm::ConnectionTrait, stock_id: &str, group_id: Uuid) {
        stock_group_member::ActiveModel {
            stock_id: Set(stock_id.into()),
            group_id: Set(group_id),
            created_at: NotSet,
        }
        .insert(db)
        .await
        .expect("seed group membership");
    }

    #[backend_test_macros::database_test]
    async fn stock_sectors_returns_jquants_group_keys_and_ignores_other_axes(
        db: crate::DatabaseHandle,
    ) {
        seed_stock(&db, "DEMO-STOCK-A", "Demo Stock A").await;
        seed_stock(&db, "DEMO-STOCK-B", "Demo Stock B").await;
        let (_, industry_group_id) = seed_group(
            &db,
            "synthetic-jquants-axis",
            "synthetic-industry",
            "Sample Industry",
            Some("jquants"),
        )
        .await;
        let (_, manual_group_id) = seed_group(
            &db,
            "synthetic-manual-axis",
            "synthetic-group",
            "Sample Group",
            None,
        )
        .await;
        seed_membership(&db, "DEMO-STOCK-A", industry_group_id).await;
        seed_membership(&db, "DEMO-STOCK-A", manual_group_id).await;
        seed_membership(&db, "DEMO-STOCK-B", manual_group_id).await;

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
                ("DEMO-STOCK-A".into(), Some("synthetic-industry".into())),
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
    async fn resolve_looks_up_group_by_axis_and_group_keys(db: crate::DatabaseHandle) {
        seed_group(&db, "demo-axis-a", "demo-group", "Other Group", None).await;
        let (group_id, _) =
            seed_group(&db, "demo-axis-b", "demo-group", "Sample Group", None).await;

        let result = build_use_cases(db)
            .resolve(&[("group".into(), group_id.clone())])
            .await
            .expect("resolve group");

        assert_eq!(
            result,
            vec![ResolvedRef {
                kind: "group".into(),
                id: group_id,
                name: Some("Sample Group".into()),
            }],
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
