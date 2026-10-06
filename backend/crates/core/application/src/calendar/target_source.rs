use std::{collections::BTreeSet, sync::Arc};

use async_trait::async_trait;

use crate::{
    stock_group::StockGroupUseCases,
    strategy_earnings_target::{StrategyEarningsTarget, StrategyEarningsTargetUseCases},
    strategy_scope::StrategyScope,
};

use super::read_use_cases::{CalendarEventTargetSource, CalendarEventTargetSourceError};

#[async_trait]
pub trait StrategyEarningsTargetReader: Send + Sync {
    async fn list_targets(
        &self,
        scope: StrategyScope,
    ) -> Result<Vec<StrategyEarningsTarget>, String>;
}

#[async_trait]
pub trait StockGroupStockIdReader: Send + Sync {
    async fn list_stock_ids(&self, axis_key: &str, group_key: &str) -> Result<Vec<String>, String>;
}

pub struct CalendarEventTargetSourceAdapter {
    strategy_earnings_targets: Arc<dyn StrategyEarningsTargetReader>,
    stock_groups: Arc<dyn StockGroupStockIdReader>,
}

impl CalendarEventTargetSourceAdapter {
    pub fn new<T, G>(strategy_earnings_targets: T, stock_groups: G) -> Self
    where
        T: StrategyEarningsTargetReader + 'static,
        G: StockGroupStockIdReader + 'static,
    {
        Self {
            strategy_earnings_targets: Arc::new(strategy_earnings_targets),
            stock_groups: Arc::new(stock_groups),
        }
    }
}

#[async_trait]
impl StrategyEarningsTargetReader for StrategyEarningsTargetUseCases {
    async fn list_targets(
        &self,
        scope: StrategyScope,
    ) -> Result<Vec<StrategyEarningsTarget>, String> {
        StrategyEarningsTargetUseCases::list(self, scope)
            .await
            .map_err(|error| error.to_string())
    }
}

#[async_trait]
impl StockGroupStockIdReader for StockGroupUseCases {
    async fn list_stock_ids(&self, axis_key: &str, group_key: &str) -> Result<Vec<String>, String> {
        StockGroupUseCases::list_stock_ids(self, axis_key, group_key)
            .await
            .map_err(|error| error.to_string())
    }
}

#[async_trait]
impl CalendarEventTargetSource for CalendarEventTargetSourceAdapter {
    async fn list_stock_ids(
        &self,
        scope: StrategyScope,
    ) -> Result<Vec<String>, CalendarEventTargetSourceError> {
        let targets = self
            .strategy_earnings_targets
            .list_targets(scope)
            .await
            .map_err(CalendarEventTargetSourceError::Failed)?;
        let mut stock_ids = BTreeSet::new();

        for target in targets {
            match target.ref_kind.as_str() {
                "stock" => {
                    stock_ids.insert(target.ref_id);
                }
                "group" => {
                    let Some((axis_key, group_key)) =
                        target
                            .ref_id
                            .split_once('/')
                            .filter(|(axis_key, group_key)| {
                                !axis_key.is_empty()
                                    && !group_key.is_empty()
                                    && !group_key.contains('/')
                            })
                    else {
                        return Err(CalendarEventTargetSourceError::Failed(format!(
                            "invalid group reference: {}",
                            target.ref_id
                        )));
                    };
                    stock_ids.extend(
                        self.stock_groups
                            .list_stock_ids(axis_key, group_key)
                            .await
                            .map_err(CalendarEventTargetSourceError::Failed)?,
                    );
                }
                ref_kind => {
                    return Err(CalendarEventTargetSourceError::Failed(format!(
                        "unknown earnings target kind: {ref_kind}"
                    )));
                }
            }
        }

        Ok(stock_ids.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use rstest::rstest;
    use uuid::Uuid;

    use crate::{
        calendar::{
            read_use_cases::{CalendarEventTargetSource, CalendarEventTargetSourceError},
            target_source::{
                CalendarEventTargetSourceAdapter, StockGroupStockIdReader,
                StrategyEarningsTargetReader,
            },
        },
        strategy_earnings_target::StrategyEarningsTarget,
        strategy_scope::{StrategyScope, StrategyScopeSource, StrategyScopeSourceError},
    };

    struct FakeStrategyEarningsTargetReader {
        targets: Vec<StrategyEarningsTarget>,
    }

    #[async_trait]
    impl StrategyEarningsTargetReader for FakeStrategyEarningsTargetReader {
        async fn list_targets(
            &self,
            _scope: StrategyScope,
        ) -> Result<Vec<StrategyEarningsTarget>, String> {
            Ok(self.targets.clone())
        }
    }

    struct FakeStockGroupStockIdReader {
        groups: HashMap<(String, String), Vec<String>>,
        calls: Arc<Mutex<Vec<(String, String)>>>,
    }

    #[async_trait]
    impl StockGroupStockIdReader for FakeStockGroupStockIdReader {
        async fn list_stock_ids(
            &self,
            axis_key: &str,
            group_key: &str,
        ) -> Result<Vec<String>, String> {
            self.calls
                .lock()
                .expect("group call lock")
                .push((axis_key.to_owned(), group_key.to_owned()));
            Ok(self
                .groups
                .get(&(axis_key.to_owned(), group_key.to_owned()))
                .cloned()
                .unwrap_or_default())
        }
    }

    struct FakeStrategyScopeSource;

    #[async_trait]
    impl StrategyScopeSource for FakeStrategyScopeSource {
        async fn existing_ids(
            &self,
            ids: &[Uuid],
        ) -> Result<HashSet<Uuid>, StrategyScopeSourceError> {
            Ok(ids.iter().copied().collect())
        }
    }

    fn target(ref_kind: &str, ref_id: &str) -> StrategyEarningsTarget {
        StrategyEarningsTarget {
            ref_kind: ref_kind.to_owned(),
            ref_id: ref_id.to_owned(),
            created_at: DateTime::<Utc>::UNIX_EPOCH.fixed_offset(),
        }
    }

    async fn strategy_scope() -> StrategyScope {
        StrategyScope::verify(Uuid::from_u128(1), &FakeStrategyScopeSource)
            .await
            .expect("known strategy")
    }

    #[tokio::test]
    async fn expands_stock_and_group_targets_and_deduplicates_members() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source = CalendarEventTargetSourceAdapter::new(
            FakeStrategyEarningsTargetReader {
                targets: vec![
                    target("stock", "SAMPLE-A"),
                    target("group", "sample-axis/sample-group"),
                    target("group", "sample-axis/other-group"),
                    target("stock", "SAMPLE-A"),
                ],
            },
            FakeStockGroupStockIdReader {
                groups: HashMap::from([
                    (
                        ("sample-axis".to_owned(), "sample-group".to_owned()),
                        vec!["SAMPLE-A".to_owned(), "SAMPLE-B".to_owned()],
                    ),
                    (
                        ("sample-axis".to_owned(), "other-group".to_owned()),
                        vec!["SAMPLE-B".to_owned(), "US:SAMPLE-C".to_owned()],
                    ),
                ]),
                calls: calls.clone(),
            },
        );

        let result = source.list_stock_ids(strategy_scope().await).await;
        let group_calls = calls.lock().expect("group calls lock").clone();

        assert_eq!(
            (result.map_err(|error| error.to_string()), group_calls),
            (
                Ok(vec![
                    "SAMPLE-A".to_owned(),
                    "SAMPLE-B".to_owned(),
                    "US:SAMPLE-C".to_owned(),
                ]),
                vec![
                    ("sample-axis".to_owned(), "sample-group".to_owned()),
                    ("sample-axis".to_owned(), "other-group".to_owned()),
                ],
            ),
        );
    }

    #[rstest]
    #[case::malformed_group(
        "group",
        "sample-axis/sample-group/extra",
        "invalid group reference: sample-axis/sample-group/extra"
    )]
    #[case::unknown_kind(
        "sample-kind",
        "sample-ref",
        "unknown earnings target kind: sample-kind"
    )]
    #[tokio::test]
    async fn rejects_malformed_group_and_unknown_target_kinds(
        #[case] ref_kind: &str,
        #[case] ref_id: &str,
        #[case] expected_error: &str,
    ) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source = CalendarEventTargetSourceAdapter::new(
            FakeStrategyEarningsTargetReader {
                targets: vec![target(ref_kind, ref_id)],
            },
            FakeStockGroupStockIdReader {
                groups: HashMap::new(),
                calls: calls.clone(),
            },
        );

        let result = source
            .list_stock_ids(strategy_scope().await)
            .await
            .map_err(|error: CalendarEventTargetSourceError| error.to_string());
        let group_calls = calls.lock().expect("group calls lock").clone();

        assert_eq!(
            (result, group_calls),
            (Err(expected_error.to_owned()), Vec::new())
        );
    }
}
