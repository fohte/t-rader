use std::collections::HashSet;
use std::time::{Duration, Instant};

use chrono::{NaiveDate, Utc};
use core_domain::calendar_event::CalendarEventReference;
use futures_util::future::join_all;
use serde::Serialize;
use thiserror::Error;

use crate::{
    calendar::{
        macro_reference_targets::{
            MacroReferenceTargetSelector, SharedMacroReferenceTargetRepository,
        },
        references::CalendarEventReferenceUseCases,
        repository::CalendarEventRepositoryError,
    },
    macro_reference_source::SharedMacroReferenceSource,
};

const MAX_RUN_DURATION: Duration = Duration::from_secs(5 * 60);
const MAX_TARGETS_PER_FETCH: usize = 25;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct MacroReferenceStats {
    pub events_checked: usize,
    pub references_upserted: usize,
    pub external_failures: usize,
    pub budget_exhausted: bool,
}

#[derive(Debug, Error)]
pub enum MacroReferenceUseCaseError {
    #[error(transparent)]
    Repository(#[from] CalendarEventRepositoryError),
    #[error(transparent)]
    Reference(#[from] crate::calendar::error::CalendarEventReferenceUseCaseError),
}

#[derive(Clone)]
pub struct MacroReferenceUseCases {
    targets: SharedMacroReferenceTargetRepository,
    references: CalendarEventReferenceUseCases,
    sources: Vec<SharedMacroReferenceSource>,
}

impl MacroReferenceUseCases {
    pub fn new(
        targets: SharedMacroReferenceTargetRepository,
        references: CalendarEventReferenceUseCases,
        sources: Vec<SharedMacroReferenceSource>,
    ) -> Self {
        Self {
            targets,
            references,
            sources,
        }
    }

    pub async fn fetch_pending(
        &self,
        today: NaiveDate,
        event_source: &str,
    ) -> Result<MacroReferenceStats, MacroReferenceUseCaseError> {
        let started_at = Instant::now();
        let Some(selector) = MacroReferenceTargetSelector::for_event_source(event_source) else {
            return Ok(MacroReferenceStats::default());
        };
        let targets = self
            .targets
            .list_pending_macro_reference_targets(today, &selector)
            .await?;
        if targets.is_empty() {
            return Ok(MacroReferenceStats::default());
        }
        let mut stats = MacroReferenceStats::default();
        for target_batch in targets.chunks(MAX_TARGETS_PER_FETCH) {
            let remaining = MAX_RUN_DURATION.saturating_sub(started_at.elapsed());
            if remaining.is_zero() {
                stats.budget_exhausted = true;
                break;
            }
            let requested_targets = target_batch
                .iter()
                .map(|target| {
                    (
                        target.event_source.clone(),
                        target.event_external_id.clone(),
                    )
                })
                .collect::<HashSet<_>>();
            let fetches = self
                .sources
                .iter()
                .filter_map(|source| {
                    let source_targets = target_batch
                        .iter()
                        .filter(|target| source.supports(target))
                        .cloned()
                        .collect::<Vec<_>>();
                    (!source_targets.is_empty()).then_some(async move {
                        let result = source.fetch_references(&source_targets).await;
                        (source_targets, result)
                    })
                })
                .collect::<Vec<_>>();
            let fetched_by_source = match tokio::time::timeout(remaining, join_all(fetches)).await {
                Ok(results) => results,
                Err(_) => {
                    stats.external_failures += target_batch.len();
                    stats.budget_exhausted = true;
                    break;
                }
            };

            let mut expected_sources = target_batch
                .iter()
                .map(|target| {
                    let key = (
                        target.event_source.clone(),
                        target.event_external_id.clone(),
                    );
                    let count = self
                        .sources
                        .iter()
                        .filter(|source| source.supports(target))
                        .count();
                    (key, count)
                })
                .collect::<std::collections::HashMap<_, _>>();
            let mut successful_sources =
                std::collections::HashMap::<(String, String), usize>::new();
            let mut failed_targets = HashSet::new();
            let mut fetched_references = Vec::new();
            for (source_targets, response) in fetched_by_source {
                let source_target_keys = source_targets
                    .iter()
                    .map(|target| {
                        (
                            target.event_source.clone(),
                            target.event_external_id.clone(),
                        )
                    })
                    .collect::<HashSet<_>>();
                let results = match response {
                    Ok(results) => results,
                    Err(error) => {
                        tracing::warn!(%error, "official macroeconomic release lookup failed");
                        failed_targets.extend(source_target_keys);
                        continue;
                    }
                };
                let mut seen_source_targets = HashSet::new();
                for result in results {
                    let key = (
                        result.target.event_source.clone(),
                        result.target.event_external_id.clone(),
                    );
                    if !source_target_keys.contains(&key)
                        || !seen_source_targets.insert(key.clone())
                    {
                        if let Some(failure) = result.failure {
                            tracing::warn!(%failure, "official macroeconomic release lookup failed");
                        }
                        continue;
                    }
                    if let Some(failure) = result.failure {
                        tracing::warn!(%failure, "official macroeconomic release lookup failed");
                        failed_targets.insert(key);
                        continue;
                    }
                    *successful_sources.entry(key).or_default() += 1;
                    fetched_references.extend(
                        result
                            .references
                            .into_iter()
                            .filter(|reference| reference.target == result.target),
                    );
                }
                failed_targets.extend(source_target_keys.difference(&seen_source_targets).cloned());
            }

            let mut checked_targets = Vec::new();
            let mut checked_keys = HashSet::new();
            for target in target_batch {
                let key = (
                    target.event_source.clone(),
                    target.event_external_id.clone(),
                );
                let expected = expected_sources.remove(&key).unwrap_or_default();
                let successful = successful_sources.get(&key).copied().unwrap_or_default();
                if expected > 0 && expected == successful && !failed_targets.contains(&key) {
                    checked_keys.insert(key);
                    checked_targets.push(target.clone());
                }
            }
            stats.external_failures += requested_targets.len().saturating_sub(checked_keys.len());
            let successful_keys = checked_targets
                .iter()
                .map(|target| {
                    (
                        target.event_source.as_str(),
                        target.event_external_id.as_str(),
                    )
                })
                .collect::<HashSet<_>>();
            let checked_at = Utc::now();
            let references = fetched_references
                .into_iter()
                .filter(|reference| {
                    successful_keys.contains(&(
                        reference.target.event_source.as_str(),
                        reference.target.event_external_id.as_str(),
                    ))
                })
                .map(|reference| CalendarEventReference {
                    event_source: reference.target.event_source,
                    event_external_id: reference.target.event_external_id,
                    kind: reference.kind,
                    source: reference.source,
                    external_id: reference.external_id,
                    title: reference.title,
                    url: reference.url,
                    target_period_start: reference.target_period_start,
                    target_period_end: reference.target_period_end,
                    target_period_type: reference.target_period_type,
                    publication_evidence: reference.publication_evidence,
                    publication_evidence_url: reference.publication_evidence_url,
                    checked_at,
                })
                .collect::<Vec<_>>();
            stats.references_upserted += self.references.upsert(references).await?;
            self.targets
                .mark_macro_reference_targets_checked(&checked_targets, checked_at)
                .await?;
            stats.events_checked += checked_targets.len();
        }
        Ok(stats)
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::{sync::Arc, time::Duration};

    use async_trait::async_trait;
    use chrono::{DateTime, NaiveDate};
    use core_domain::calendar_event::{
        CalendarEvent, CalendarEventReadModel, CalendarEventReference,
    };
    use rstest::fixture;

    use crate::{
        calendar::{
            macro_reference_targets::{
                MacroReferenceTarget, MacroReferenceTargetRepository, MacroReferenceTargetSelector,
            },
            references::CalendarEventReferenceUseCases,
            repository::{
                CalendarEventRepository, CalendarEventRepositoryError,
                SharedCalendarEventRepository,
            },
        },
        daily_bar_source::DateRange,
        macro_reference_source::{
            MacroReferenceSource, MacroReferenceSourceError, SharedMacroReferenceSource,
        },
        unit_of_work::{FakeUnitOfWork, SharedUnitOfWork, UnitOfWorkTransaction},
    };

    use super::{MacroReferenceStats, MacroReferenceUseCases};

    struct FakeTargetRepository {
        targets: Vec<MacroReferenceTarget>,
        checked: tokio::sync::Mutex<Vec<MacroReferenceTarget>>,
    }

    #[async_trait]
    impl MacroReferenceTargetRepository for FakeTargetRepository {
        async fn list_pending_macro_reference_targets(
            &self,
            _today: NaiveDate,
            _selector: &MacroReferenceTargetSelector,
        ) -> Result<Vec<MacroReferenceTarget>, CalendarEventRepositoryError> {
            Ok(self.targets.clone())
        }

        async fn mark_macro_reference_targets_checked(
            &self,
            targets: &[MacroReferenceTarget],
            _checked_at: DateTime<chrono::Utc>,
        ) -> Result<(), CalendarEventRepositoryError> {
            self.checked.lock().await.extend_from_slice(targets);
            Ok(())
        }
    }

    struct FakeCalendarRepository;

    #[async_trait]
    impl CalendarEventRepository for FakeCalendarRepository {
        async fn list_events(
            &self,
            _date_range: &DateRange,
        ) -> Result<Vec<CalendarEvent>, CalendarEventRepositoryError> {
            Ok(Vec::new())
        }

        async fn enrich_events(
            &self,
            events: Vec<CalendarEvent>,
        ) -> Result<Vec<CalendarEventReadModel>, CalendarEventRepositoryError> {
            Ok(events.into_iter().map(Into::into).collect())
        }

        async fn list_references_for_event(
            &self,
            _event_source: &str,
            _event_external_id: &str,
        ) -> Result<Vec<CalendarEventReference>, CalendarEventRepositoryError> {
            Ok(Vec::new())
        }

        async fn find_next_earnings_for_stock(
            &self,
            _stock_id: &str,
            _from: NaiveDate,
        ) -> Result<Option<CalendarEvent>, CalendarEventRepositoryError> {
            Ok(None)
        }

        async fn upsert(
            &self,
            _transaction: &UnitOfWorkTransaction,
            events: Vec<CalendarEvent>,
        ) -> Result<usize, CalendarEventRepositoryError> {
            Ok(events.len())
        }

        async fn upsert_references(
            &self,
            _transaction: &UnitOfWorkTransaction,
            references: Vec<CalendarEventReference>,
        ) -> Result<usize, CalendarEventRepositoryError> {
            Ok(references.len())
        }

        async fn delete_missing_future_events(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _source: &str,
            _date_range: &DateRange,
            _today: NaiveDate,
            _external_ids: Vec<String>,
        ) -> Result<u64, CalendarEventRepositoryError> {
            Ok(0)
        }
    }

    struct FixedMacroSource {
        failure: bool,
        failure_target: Option<String>,
        delay: Duration,
        supported_event_source: Option<String>,
    }

    #[async_trait]
    impl MacroReferenceSource for FixedMacroSource {
        fn supports(&self, target: &MacroReferenceTarget) -> bool {
            self.supported_event_source
                .as_deref()
                .is_none_or(|source| source == target.event_source)
        }

        async fn fetch_references(
            &self,
            targets: &[MacroReferenceTarget],
        ) -> Result<
            Vec<crate::macro_reference_source::MacroReferenceFetchResult>,
            MacroReferenceSourceError,
        > {
            tokio::time::sleep(self.delay).await;
            Ok(targets
                .iter()
                .cloned()
                .map(|target| {
                    if self.failure
                        || self.failure_target.as_deref() == Some(target.event_external_id.as_str())
                    {
                        crate::macro_reference_source::MacroReferenceFetchResult::retry(
                            target,
                            "synthetic transport failure",
                        )
                    } else {
                        crate::macro_reference_source::MacroReferenceFetchResult::success(
                            target,
                            Vec::new(),
                        )
                    }
                })
                .collect())
        }
    }

    #[fixture]
    fn target() -> MacroReferenceTarget {
        MacroReferenceTarget {
            event_source: "e_stat".to_owned(),
            event_external_id: "sample-event-2099-04-08".to_owned(),
            event_title: "架空の統計イベント".to_owned(),
            same_day_event_count: 1,
        }
    }

    fn use_cases(
        targets: Vec<MacroReferenceTarget>,
        source: FixedMacroSource,
    ) -> (MacroReferenceUseCases, Arc<FakeTargetRepository>) {
        use_cases_with_sources(targets, vec![Arc::new(source)])
    }

    fn use_cases_with_sources(
        targets: Vec<MacroReferenceTarget>,
        sources: Vec<SharedMacroReferenceSource>,
    ) -> (MacroReferenceUseCases, Arc<FakeTargetRepository>) {
        let target_repository = Arc::new(FakeTargetRepository {
            targets,
            checked: tokio::sync::Mutex::new(Vec::new()),
        });
        let calendar_repository: SharedCalendarEventRepository = Arc::new(FakeCalendarRepository);
        let unit_of_work: SharedUnitOfWork = Arc::new(FakeUnitOfWork::new());
        let references = CalendarEventReferenceUseCases::new(calendar_repository, unit_of_work);
        (
            MacroReferenceUseCases::new(target_repository.clone(), references, sources),
            target_repository,
        )
    }

    #[tokio::test]
    async fn successful_lookup_marks_no_document_targets_checked() {
        let target = target();
        let (use_cases, repository) = use_cases(
            vec![target.clone()],
            FixedMacroSource {
                failure: false,
                failure_target: None,
                delay: Duration::ZERO,
                supported_event_source: None,
            },
        );

        let stats = use_cases
            .fetch_pending(
                NaiveDate::from_ymd_opt(2099, 4, 9).unwrap_or(NaiveDate::MIN),
                "e_stat",
            )
            .await
            .unwrap_or_default();
        let actual = (stats, repository.checked.lock().await.clone());

        assert_eq!(
            actual,
            (
                MacroReferenceStats {
                    events_checked: 1,
                    ..MacroReferenceStats::default()
                },
                vec![target],
            ),
        );
    }

    #[tokio::test]
    async fn each_target_uses_only_the_sources_that_support_it() {
        let first = target();
        let second = MacroReferenceTarget {
            event_source: "sample_other_calendar".to_owned(),
            event_external_id: "sample-other-event-2099-04-08".to_owned(),
            event_title: "架空の統計イベント".to_owned(),
            same_day_event_count: 1,
        };
        let sources: Vec<SharedMacroReferenceSource> = vec![
            Arc::new(FixedMacroSource {
                failure: false,
                failure_target: None,
                delay: Duration::ZERO,
                supported_event_source: Some(first.event_source.clone()),
            }),
            Arc::new(FixedMacroSource {
                failure: false,
                failure_target: None,
                delay: Duration::ZERO,
                supported_event_source: Some(second.event_source.clone()),
            }),
        ];
        let (use_cases, repository) =
            use_cases_with_sources(vec![first.clone(), second.clone()], sources);

        let stats = use_cases
            .fetch_pending(
                NaiveDate::from_ymd_opt(2099, 4, 9).unwrap_or(NaiveDate::MIN),
                "e_stat",
            )
            .await
            .unwrap_or_default();
        let actual = (stats, repository.checked.lock().await.clone());

        assert_eq!(
            actual,
            (
                MacroReferenceStats {
                    events_checked: 2,
                    ..MacroReferenceStats::default()
                },
                vec![first, second],
            ),
        );
    }

    #[rstest::rstest]
    #[case::external_failure(Duration::ZERO, true, false)]
    #[case::timeout(Duration::from_secs(6 * 60), false, true)]
    #[tokio::test(start_paused = true)]
    async fn failed_lookup_does_not_mark_targets_checked(
        #[case] delay: Duration,
        #[case] failure: bool,
        #[case] budget_exhausted: bool,
    ) {
        let target = target();
        let (use_cases, repository) = use_cases(
            vec![target],
            FixedMacroSource {
                failure,
                failure_target: None,
                delay,
                supported_event_source: None,
            },
        );

        let stats = use_cases
            .fetch_pending(
                NaiveDate::from_ymd_opt(2099, 4, 9).unwrap_or(NaiveDate::MIN),
                "e_stat",
            )
            .await
            .unwrap_or_default();
        let actual = (stats, repository.checked.lock().await.clone());

        assert_eq!(
            actual,
            (
                MacroReferenceStats {
                    external_failures: 1,
                    budget_exhausted,
                    ..MacroReferenceStats::default()
                },
                Vec::new(),
            ),
        );
    }

    #[tokio::test]
    async fn completed_batches_are_checked_before_a_later_batch_fails() {
        let targets = (0..26)
            .map(|index| MacroReferenceTarget {
                event_source: "e_stat".to_owned(),
                event_external_id: format!("sample-event-{index:02}-2099-04-08"),
                event_title: "架空の統計イベント".to_owned(),
                same_day_event_count: 1,
            })
            .collect::<Vec<_>>();
        let failure_target = targets[25].event_external_id.clone();
        let expected_checked = targets[..25].to_vec();
        let (use_cases, repository) = use_cases(
            targets,
            FixedMacroSource {
                failure: false,
                failure_target: Some(failure_target),
                delay: Duration::ZERO,
                supported_event_source: None,
            },
        );

        let stats = use_cases
            .fetch_pending(
                NaiveDate::from_ymd_opt(2099, 4, 9).unwrap_or(NaiveDate::MIN),
                "e_stat",
            )
            .await
            .unwrap_or_default();
        let actual = (stats, repository.checked.lock().await.clone());

        assert_eq!(
            actual,
            (
                MacroReferenceStats {
                    events_checked: 25,
                    external_failures: 1,
                    ..MacroReferenceStats::default()
                },
                expected_checked,
            ),
        );
    }
}
