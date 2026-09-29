use chrono::Utc;
use rust_decimal::Decimal;
use serde_json::{Map, json};
use uuid::Uuid;

use crate::change_history::{Actor, ChangeHistoryRecord, Op, TargetKind};
use crate::strategy_scope::StrategyScope;
use crate::unit_of_work::SharedUnitOfWork;

use super::error::StrategyUseCaseError;
use super::query::SharedStrategySummaryQuery;
use super::repository::SharedStrategyRepository;
use super::types::{
    CreateStrategyCommand, NewInvestableAmount, NewStrategy, Strategy, StrategySummary,
    StrategyUpdateCommand,
};

#[derive(Clone)]
pub struct StrategyUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedStrategyRepository,
    summary_query: SharedStrategySummaryQuery,
    change_history: crate::change_history::SharedChangeHistoryPort,
}

impl StrategyUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedStrategyRepository,
        summary_query: SharedStrategySummaryQuery,
        change_history: crate::change_history::SharedChangeHistoryPort,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            summary_query,
            change_history,
        }
    }

    pub async fn list(&self) -> Result<Vec<Strategy>, StrategyUseCaseError> {
        self.repository.list().await.map_err(Into::into)
    }

    pub async fn list_summaries(&self) -> Result<Vec<StrategySummary>, StrategyUseCaseError> {
        self.summary_query.list().await.map_err(Into::into)
    }

    pub async fn get(&self, scope: StrategyScope) -> Result<Strategy, StrategyUseCaseError> {
        self.repository
            .find_by_id(scope.id())
            .await?
            .ok_or(StrategyUseCaseError::NotFound(scope.id()))
    }

    pub async fn create(
        &self,
        actor: Actor,
        command: CreateStrategyCommand,
    ) -> Result<Strategy, StrategyUseCaseError> {
        let name = validate_name(&command.name)?;
        let id = Uuid::new_v4();
        let transaction = self.unit_of_work.begin().await?;
        let strategy = self
            .repository
            .create(
                &transaction,
                NewStrategy {
                    id,
                    name: name.clone(),
                    description: command.description,
                    sort_order: command.sort_order,
                },
            )
            .await?;
        self.change_history
            .record(
                &transaction,
                ChangeHistoryRecord {
                    actor,
                    target_kind: TargetKind::Strategy,
                    target_id: id,
                    op: Op::Create,
                    diff: json!({
                        "name": strategy.name,
                        "sort_order": strategy.sort_order,
                        "description": strategy.description,
                    }),
                    summary: Some(format!("created strategy {name}")),
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(strategy)
    }

    pub async fn update(
        &self,
        actor: Actor,
        scope: StrategyScope,
        command: StrategyUpdateCommand,
    ) -> Result<Strategy, StrategyUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let current = self
            .repository
            .find_by_id_in_transaction(&transaction, scope.id())
            .await?
            .ok_or(StrategyUseCaseError::NotFound(scope.id()))?;
        let mut updated = current.clone();
        let mut diff = Map::new();

        if let Some(name) = command.name {
            let name = validate_name(&name)?;
            diff.insert("name".into(), json!({ "from": current.name, "to": name }));
            updated.name = name;
        }
        if let Some(description) = command.description {
            diff.insert(
                "description".into(),
                json!({ "from": current.description, "to": description }),
            );
            updated.description = description;
        }
        if let Some(sort_order) = command.sort_order {
            diff.insert(
                "sort_order".into(),
                json!({ "from": current.sort_order, "to": sort_order }),
            );
            updated.sort_order = sort_order;
        }
        updated.updated_at = Utc::now().fixed_offset();

        let updated = self.repository.update(&transaction, updated).await?;
        if !diff.is_empty() {
            self.change_history
                .record(
                    &transaction,
                    ChangeHistoryRecord {
                        actor,
                        target_kind: TargetKind::Strategy,
                        target_id: scope.id(),
                        op: Op::Update,
                        diff: serde_json::Value::Object(diff),
                        summary: None,
                    },
                )
                .await?;
        }
        self.unit_of_work.commit(transaction).await?;
        Ok(updated)
    }

    pub async fn delete(
        &self,
        actor: Actor,
        scope: StrategyScope,
    ) -> Result<(), StrategyUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        if !self.repository.delete(&transaction, scope.id()).await? {
            return Err(StrategyUseCaseError::NotFound(scope.id()));
        }
        self.record_delete(&transaction, actor, scope.id()).await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }

    pub async fn delete_confirmed(
        &self,
        actor: Actor,
        scope: StrategyScope,
        expected_name: &str,
    ) -> Result<(), StrategyUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        if !self
            .repository
            .delete_confirmed(&transaction, scope.id(), expected_name)
            .await?
        {
            return Err(StrategyUseCaseError::ConfirmationMismatch(scope.id()));
        }
        self.record_delete(&transaction, actor, scope.id()).await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }

    pub async fn current_investable_amount(
        &self,
        scope: StrategyScope,
    ) -> Result<Option<super::types::InvestableAmount>, StrategyUseCaseError> {
        self.repository
            .find_by_id(scope.id())
            .await?
            .ok_or(StrategyUseCaseError::NotFound(scope.id()))?;
        self.repository
            .find_current_investable_amount(scope.id(), Utc::now().fixed_offset())
            .await
            .map_err(Into::into)
    }

    pub async fn record_investable_amount(
        &self,
        actor: Actor,
        scope: StrategyScope,
        amount_jpy: Decimal,
        effective_at: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<super::types::InvestableAmount, StrategyUseCaseError> {
        if amount_jpy < Decimal::ZERO {
            return Err(StrategyUseCaseError::Validation(
                "amount_jpy must be non-negative".into(),
            ));
        }

        let transaction = self.unit_of_work.begin().await?;
        self.repository
            .find_by_id_in_transaction(&transaction, scope.id())
            .await?
            .ok_or(StrategyUseCaseError::NotFound(scope.id()))?;
        let amount = self
            .repository
            .record_investable_amount(
                &transaction,
                NewInvestableAmount {
                    id: Uuid::new_v4(),
                    strategy_id: scope.id(),
                    amount_jpy,
                    effective_at,
                },
            )
            .await?;
        self.change_history
            .record(
                &transaction,
                ChangeHistoryRecord {
                    actor,
                    target_kind: TargetKind::Strategy,
                    target_id: scope.id(),
                    op: Op::Update,
                    diff: json!({
                        "investable_amount": {
                            "amount_jpy": amount.amount_jpy,
                            "effective_at": amount.effective_at,
                        },
                    }),
                    summary: None,
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(amount)
    }

    async fn record_delete(
        &self,
        transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        actor: Actor,
        strategy_id: Uuid,
    ) -> Result<(), StrategyUseCaseError> {
        self.change_history
            .record(
                transaction,
                ChangeHistoryRecord {
                    actor,
                    target_kind: TargetKind::Strategy,
                    target_id: strategy_id,
                    op: Op::Delete,
                    diff: json!({}),
                    summary: None,
                },
            )
            .await?;
        Ok(())
    }
}

pub fn validate_name(value: &str) -> Result<String, StrategyUseCaseError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(StrategyUseCaseError::Validation(
            "name must not be empty".into(),
        ));
    }
    Ok(trimmed)
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use crate::change_history::{
        Actor, ChangeHistoryRecord, FakeChangeHistory, FakeChangeHistoryEntry, Op, TargetKind,
    };
    use crate::strategy::FakeStrategyRepository;
    use crate::strategy::FakeStrategySummaryQuery;
    use crate::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork};

    use super::{CreateStrategyCommand, StrategyUseCases};

    #[tokio::test]
    async fn create_records_strategy_and_history_in_the_same_transaction() {
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let repository = Arc::new(FakeStrategyRepository::new());
        let summary_query = Arc::new(FakeStrategySummaryQuery::new());
        let change_history = Arc::new(FakeChangeHistory::new());
        let use_cases = StrategyUseCases::new(
            unit_of_work.clone() as SharedUnitOfWork,
            repository.clone(),
            summary_query,
            change_history.clone(),
        );

        let created = use_cases
            .create(
                Actor::Human,
                CreateStrategyCommand {
                    name: "  sample strategy  ".into(),
                    description: Some("sample description".into()),
                    sort_order: 2,
                },
            )
            .await
            .expect("create strategy");

        let transaction_id = unit_of_work.begun.lock().await[0];
        assert_eq!(
            (
                unit_of_work.begun.lock().await.clone(),
                unit_of_work.committed.lock().await.clone(),
                repository.transaction_ids.lock().await.clone(),
                change_history.entries.lock().await.clone(),
            ),
            (
                vec![transaction_id],
                vec![transaction_id],
                vec![transaction_id],
                vec![FakeChangeHistoryEntry {
                    transaction_id,
                    record: ChangeHistoryRecord {
                        actor: Actor::Human,
                        target_kind: TargetKind::Strategy,
                        target_id: created.id,
                        op: Op::Create,
                        diff: json!({
                            "name": "sample strategy",
                            "sort_order": 2,
                            "description": "sample description",
                        }),
                        summary: Some("created strategy sample strategy".into()),
                    },
                }],
            ),
        );
    }
}
