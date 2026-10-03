use serde_json::json;
use uuid::Uuid;

use core_domain::stock_id::ForeignStockId;

use crate::change_history::{Actor, ChangeHistoryRecord, Op, SharedChangeHistoryPort, TargetKind};
use crate::persistence::PersistenceError;
use crate::unit_of_work::SharedUnitOfWork;

use super::error::StockRegistrationUseCaseError;
use super::repository::{SharedStockRegistrationRepository, StockRegistrationRepositoryError};
use super::types::{NewStockRegistration, RegisterStockCommand, RegisteredStock};

#[derive(Clone)]
pub struct StockRegistrationUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedStockRegistrationRepository,
    change_history: SharedChangeHistoryPort,
}

impl StockRegistrationUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedStockRegistrationRepository,
        change_history: SharedChangeHistoryPort,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            change_history,
        }
    }

    pub async fn register(
        &self,
        command: RegisterStockCommand,
    ) -> Result<RegisteredStock, StockRegistrationUseCaseError> {
        let id = ForeignStockId::new(&command.country, &command.code)
            .map_err(|error| StockRegistrationUseCaseError::Validation(error.to_string()))?;
        let name = validate_required("name", &command.name)?;
        let exchange = validate_required("exchange", &command.exchange)?;
        let instrument_market = id.market();
        let transaction = self.unit_of_work.begin().await?;

        self.repository
            .insert(
                &transaction,
                NewStockRegistration {
                    id: id.clone(),
                    name: name.clone(),
                    exchange: exchange.clone(),
                    instrument_market,
                },
            )
            .await
            .map_err(|error| match error {
                StockRegistrationRepositoryError::Database(PersistenceError::Conflict(_)) => {
                    StockRegistrationUseCaseError::Validation(format!(
                        "stock {} already exists",
                        id.as_str()
                    ))
                }
                error => error.into(),
            })?;

        self.change_history
            .record(
                &transaction,
                ChangeHistoryRecord {
                    actor: Actor::Llm { label: "analyst" },
                    target_kind: TargetKind::Stock,
                    target_id: history_target_id(id.as_str()),
                    op: Op::Create,
                    diff: json!({
                        "stock_id": id.as_str(),
                        "name": name,
                        "exchange": exchange,
                    }),
                    summary: None,
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;

        Ok(RegisteredStock {
            id: id.as_str().to_owned(),
            name,
            exchange,
        })
    }
}

fn validate_required(field: &str, value: &str) -> Result<String, StockRegistrationUseCaseError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(StockRegistrationUseCaseError::Validation(format!(
            "{field} must not be empty"
        )));
    }
    Ok(value.to_owned())
}

fn history_target_id(stock_id: &str) -> Uuid {
    // change_history の target_id は UUID のため、文字列 ID から安定して導出する。
    Uuid::new_v5(&Uuid::NAMESPACE_OID, format!("stock:{stock_id}").as_bytes())
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::Arc;

    use core_domain::instrument::Market;
    use core_domain::stock_id::ForeignStockId;
    use rstest::{fixture, rstest};
    use serde_json::json;
    use uuid::Uuid;

    use crate::change_history::{
        Actor, ChangeHistoryRecord, FakeChangeHistory, FakeChangeHistoryEntry, Op, TargetKind,
    };
    use crate::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork};

    use super::{
        RegisterStockCommand, RegisteredStock, StockRegistrationUseCases, history_target_id,
    };
    use crate::stock_registration::{
        FakeStockRegistrationRepository, NewStockRegistration, SharedStockRegistrationRepository,
    };

    struct Harness {
        use_cases: StockRegistrationUseCases,
        repository: Arc<FakeStockRegistrationRepository>,
        unit_of_work: Arc<FakeUnitOfWork>,
        change_history: Arc<FakeChangeHistory>,
    }

    #[fixture]
    fn harness() -> Harness {
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let repository = Arc::new(FakeStockRegistrationRepository::new());
        let change_history = Arc::new(FakeChangeHistory::new());
        let shared_unit_of_work: SharedUnitOfWork = unit_of_work.clone();
        let shared_repository: SharedStockRegistrationRepository = repository.clone();

        Harness {
            use_cases: StockRegistrationUseCases::new(
                shared_unit_of_work,
                shared_repository,
                change_history.clone(),
            ),
            repository,
            unit_of_work,
            change_history,
        }
    }

    fn command(country: &str, code: &str) -> RegisterStockCommand {
        RegisterStockCommand {
            country: country.to_owned(),
            code: code.to_owned(),
            name: "Sample issuer".into(),
            exchange: "Synthetic exchange".into(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn register_persists_both_market_records_and_audit_history(harness: Harness) {
        let result = harness
            .use_cases
            .register(command("KR", "QZ9012"))
            .await
            .expect("register foreign stock");
        let registered = harness.repository.find("KR:QZ9012").await;
        let history = harness
            .change_history
            .entries
            .lock()
            .await
            .iter()
            .map(|entry| FakeChangeHistoryEntry {
                transaction_id: Uuid::nil(),
                record: entry.record.clone(),
            })
            .collect::<Vec<_>>();
        let committed = harness.unit_of_work.committed.lock().await.len();

        assert_eq!(
            (result, registered, history, committed),
            (
                RegisteredStock {
                    id: "KR:QZ9012".into(),
                    name: "Sample issuer".into(),
                    exchange: "Synthetic exchange".into(),
                },
                Some(NewStockRegistration {
                    id: ForeignStockId::new("KR", "QZ9012").expect("valid stock ID"),
                    name: "Sample issuer".into(),
                    exchange: "Synthetic exchange".into(),
                    instrument_market: Market::Other,
                }),
                vec![FakeChangeHistoryEntry {
                    transaction_id: Uuid::nil(),
                    record: ChangeHistoryRecord {
                        actor: Actor::Llm { label: "analyst" },
                        target_kind: TargetKind::Stock,
                        target_id: history_target_id("KR:QZ9012"),
                        op: Op::Create,
                        diff: json!({
                            "stock_id": "KR:QZ9012",
                            "name": "Sample issuer",
                            "exchange": "Synthetic exchange",
                        }),
                        summary: None,
                    },
                }],
                1,
            ),
        );
    }

    #[rstest]
    #[case::empty_name("", "Synthetic exchange", "name must not be empty")]
    #[case::whitespace_name("   ", "Synthetic exchange", "name must not be empty")]
    #[case::empty_exchange("Sample issuer", "", "exchange must not be empty")]
    #[case::whitespace_exchange("Sample issuer", "   ", "exchange must not be empty")]
    #[tokio::test]
    async fn register_rejects_empty_required_fields_before_starting_a_transaction(
        harness: Harness,
        #[case] name: &str,
        #[case] exchange: &str,
        #[case] expected_error: &str,
    ) {
        let mut command = command("KR", "QZ9012");
        command.name = name.to_owned();
        command.exchange = exchange.to_owned();
        let result = harness
            .use_cases
            .register(command)
            .await
            .map_err(|error| error.to_string());
        let transaction_count = harness.unit_of_work.begun.lock().await.len();
        let no_registered_stocks = harness.repository.is_empty().await;
        let history_count = harness.change_history.entries.lock().await.len();

        assert_eq!(
            (
                result,
                transaction_count,
                no_registered_stocks,
                history_count,
            ),
            (Err(expected_error.into()), 0, true, 0),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn register_rejects_duplicate_ids_without_committing_or_recording_history(
        harness: Harness,
    ) {
        let first = harness
            .use_cases
            .register(command("KR", "QZ9012"))
            .await
            .map_err(|error| error.to_string());
        let duplicate = harness
            .use_cases
            .register(command("KR", "QZ9012"))
            .await
            .map_err(|error| error.to_string());
        let stored_stock = harness.repository.find("KR:QZ9012").await;
        let history_count = harness.change_history.entries.lock().await.len();
        let committed_count = harness.unit_of_work.committed.lock().await.len();
        let begun_count = harness.unit_of_work.begun.lock().await.len();

        assert_eq!(
            (
                first,
                duplicate,
                stored_stock,
                history_count,
                committed_count,
                begun_count,
            ),
            (
                Ok(RegisteredStock {
                    id: "KR:QZ9012".into(),
                    name: "Sample issuer".into(),
                    exchange: "Synthetic exchange".into(),
                }),
                Err("stock KR:QZ9012 already exists".into()),
                Some(NewStockRegistration {
                    id: ForeignStockId::new("KR", "QZ9012").expect("valid stock ID"),
                    name: "Sample issuer".into(),
                    exchange: "Synthetic exchange".into(),
                    instrument_market: Market::Other,
                }),
                1,
                1,
                2,
            ),
        );
    }

    #[rstest]
    #[case::japan("JP", "QZ9012", "Japanese stocks must not use a country prefix")]
    #[case::lowercase_code(
        "US",
        "qz9012",
        "code must contain only uppercase ASCII letters, digits, or hyphens"
    )]
    #[case::invalid_country("U1", "QZ9012", "country must be an assigned ISO 3166-1 alpha-2 code")]
    #[case::unassigned_country(
        "ZZ",
        "QZ9012",
        "country must be an assigned ISO 3166-1 alpha-2 code"
    )]
    #[tokio::test]
    async fn register_rejects_invalid_ids_before_starting_a_transaction(
        harness: Harness,
        #[case] country: &str,
        #[case] code: &str,
        #[case] expected_error: &str,
    ) {
        let result = harness
            .use_cases
            .register(command(country, code))
            .await
            .map_err(|error| error.to_string());
        let transaction_count = harness.unit_of_work.begun.lock().await.len();
        let no_registered_stocks = harness.repository.is_empty().await;
        let history_count = harness.change_history.entries.lock().await.len();

        assert_eq!(
            (
                result,
                transaction_count,
                no_registered_stocks,
                history_count,
            ),
            (Err(expected_error.into()), 0, true, 0),
        );
    }
}
