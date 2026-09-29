use chrono::Utc;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::change_history::{Actor, ChangeHistoryRecord, Op, SharedChangeHistoryPort, TargetKind};
use crate::strategy_existence::SharedStrategyExistence;
use crate::strategy_scope::StrategyScope;
use crate::unit_of_work::{SharedUnitOfWork, UnitOfWorkTransaction};

use super::error::CustomIndicatorUseCaseError;
use super::repository::SharedCustomIndicatorRepository;
use super::types::{
    CreateCustomIndicatorCommand, CustomIndicator, NewCustomIndicator, SCOPE_GLOBAL,
    SCOPE_STRATEGY, UpdateCustomIndicatorCommand,
};

#[derive(Clone)]
pub struct CustomIndicatorUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedCustomIndicatorRepository,
    strategy_existence: SharedStrategyExistence,
    change_history: SharedChangeHistoryPort,
}

impl CustomIndicatorUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedCustomIndicatorRepository,
        strategy_existence: SharedStrategyExistence,
        change_history: SharedChangeHistoryPort,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            strategy_existence,
            change_history,
        }
    }

    pub async fn list_global(&self) -> Result<Vec<CustomIndicator>, CustomIndicatorUseCaseError> {
        self.repository.list_global().await.map_err(Into::into)
    }

    pub async fn list_strategy(
        &self,
        strategy_id: Uuid,
    ) -> Result<Vec<CustomIndicator>, CustomIndicatorUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        self.ensure_strategy_exists(&transaction, strategy_id)
            .await?;
        let indicators = self
            .repository
            .list_strategy(&transaction, strategy_id)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(indicators)
    }

    pub async fn get(
        &self,
        indicator_id: Uuid,
    ) -> Result<CustomIndicator, CustomIndicatorUseCaseError> {
        self.repository
            .find_by_id(indicator_id)
            .await?
            .ok_or(CustomIndicatorUseCaseError::NotFound(indicator_id))
    }

    pub async fn get_strategy_indicator(
        &self,
        strategy_id: Uuid,
        indicator_id: Uuid,
    ) -> Result<CustomIndicator, CustomIndicatorUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        self.ensure_strategy_exists(&transaction, strategy_id)
            .await?;
        let indicator = self
            .repository
            .find_by_id_in_transaction(&transaction, indicator_id)
            .await?
            .filter(|indicator| {
                indicator.scope == SCOPE_STRATEGY && indicator.strategy_id == Some(strategy_id)
            })
            .ok_or(CustomIndicatorUseCaseError::NotFound(indicator_id))?;
        self.unit_of_work.commit(transaction).await?;
        Ok(indicator)
    }

    pub async fn resolve(
        &self,
        scope: StrategyScope,
        name: &str,
    ) -> Result<Option<CustomIndicator>, CustomIndicatorUseCaseError> {
        self.repository
            .resolve_for_strategy(scope.id(), name)
            .await
            .map_err(Into::into)
    }

    pub async fn create(
        &self,
        command: CreateCustomIndicatorCommand,
    ) -> Result<CustomIndicator, CustomIndicatorUseCaseError> {
        let name = validate_name(&command.name)?;
        ensure_json_object("input_schema", &command.input_schema)?;
        ensure_json_object("output_schema", &command.output_schema)?;

        let transaction = self.unit_of_work.begin().await?;
        if let Some(strategy_id) = command.strategy_id {
            self.ensure_strategy_exists(&transaction, strategy_id)
                .await?;
        }
        let scope = if command.strategy_id.is_some() {
            SCOPE_STRATEGY
        } else {
            SCOPE_GLOBAL
        };
        let indicator_id = Uuid::new_v4();
        let indicator = self
            .repository
            .insert(
                &transaction,
                NewCustomIndicator {
                    indicator_id,
                    name: name.clone(),
                    scope: scope.into(),
                    strategy_id: command.strategy_id,
                    code: command.code,
                    input_schema: command.input_schema,
                    output_schema: command.output_schema,
                    description: command.description,
                },
            )
            .await?;
        self.change_history
            .record(
                &transaction,
                ChangeHistoryRecord {
                    actor: Actor::Human,
                    target_kind: TargetKind::CustomIndicator,
                    target_id: indicator_id,
                    op: Op::Create,
                    diff: json!({
                        "name": name,
                        "scope": scope,
                        "strategy_id": command.strategy_id,
                    }),
                    summary: None,
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(indicator)
    }

    pub async fn update(
        &self,
        indicator_id: Uuid,
        command: UpdateCustomIndicatorCommand,
    ) -> Result<CustomIndicator, CustomIndicatorUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let current = self
            .repository
            .find_by_id_in_transaction(&transaction, indicator_id)
            .await?
            .ok_or(CustomIndicatorUseCaseError::NotFound(indicator_id))?;
        let mut next = current.clone();
        let mut diff = Map::new();

        if let Some(name) = command.name {
            let name = validate_name(&name)?;
            diff.insert("name".into(), json!({ "from": current.name, "to": name }));
            next.name = name;
        }
        if let Some(code) = command.code {
            diff.insert(
                "code".into(),
                json!({ "len_from": current.code.len(), "len_to": code.len() }),
            );
            next.code = code;
        }
        if let Some(input_schema) = command.input_schema {
            ensure_json_object("input_schema", &input_schema)?;
            diff.insert(
                "input_schema".into(),
                json!({ "from": current.input_schema, "to": input_schema }),
            );
            next.input_schema = input_schema;
        }
        if let Some(output_schema) = command.output_schema {
            ensure_json_object("output_schema", &output_schema)?;
            diff.insert(
                "output_schema".into(),
                json!({ "from": current.output_schema, "to": output_schema }),
            );
            next.output_schema = output_schema;
        }
        if let Some(description) = command.description {
            diff.insert(
                "description".into(),
                json!({ "from": current.description, "to": description }),
            );
            next.description = description;
        }
        next.updated_at = Utc::now().fixed_offset();

        let updated = self.repository.update(&transaction, next).await?;
        if !diff.is_empty() {
            self.change_history
                .record(
                    &transaction,
                    ChangeHistoryRecord {
                        actor: Actor::Human,
                        target_kind: TargetKind::CustomIndicator,
                        target_id: indicator_id,
                        op: Op::Update,
                        diff: Value::Object(diff),
                        summary: None,
                    },
                )
                .await?;
        }
        self.unit_of_work.commit(transaction).await?;
        Ok(updated)
    }

    pub async fn delete(&self, indicator_id: Uuid) -> Result<(), CustomIndicatorUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        if !self.repository.delete(&transaction, indicator_id).await? {
            return Err(CustomIndicatorUseCaseError::NotFound(indicator_id));
        }
        self.change_history
            .record(
                &transaction,
                ChangeHistoryRecord {
                    actor: Actor::Human,
                    target_kind: TargetKind::CustomIndicator,
                    target_id: indicator_id,
                    op: Op::Delete,
                    diff: json!({}),
                    summary: None,
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }

    async fn ensure_strategy_exists(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
    ) -> Result<(), CustomIndicatorUseCaseError> {
        if !self
            .strategy_existence
            .exists(transaction, strategy_id)
            .await?
        {
            return Err(CustomIndicatorUseCaseError::Validation(format!(
                "strategy {strategy_id} does not exist"
            )));
        }
        Ok(())
    }
}

fn validate_name(value: &str) -> Result<String, CustomIndicatorUseCaseError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(CustomIndicatorUseCaseError::Validation(
            "name must not be empty".into(),
        ));
    }
    Ok(trimmed)
}

fn ensure_json_object(field: &str, value: &Value) -> Result<(), CustomIndicatorUseCaseError> {
    if value.is_object() {
        Ok(())
    } else {
        Err(CustomIndicatorUseCaseError::Validation(format!(
            "{field} must be a JSON object"
        )))
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::Arc;

    use serde_json::json;
    use uuid::Uuid;

    use crate::change_history::{Actor, FakeChangeHistory, Op, TargetKind};
    use crate::strategy_existence::FakeStrategyExistence;
    use crate::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork};

    use super::{CreateCustomIndicatorCommand, CustomIndicatorUseCases};
    use crate::custom_indicator::FakeCustomIndicatorRepository;

    #[tokio::test]
    async fn create_records_indicator_and_history_in_one_transaction() {
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let repository = Arc::new(FakeCustomIndicatorRepository::new());
        let strategy_existence = Arc::new(FakeStrategyExistence::new());
        let change_history = Arc::new(FakeChangeHistory::new());
        let strategy_id = Uuid::nil();
        strategy_existence.insert_strategy(strategy_id).await;
        let shared_unit_of_work: SharedUnitOfWork = unit_of_work.clone();
        let use_cases = CustomIndicatorUseCases::new(
            shared_unit_of_work,
            repository.clone(),
            strategy_existence.clone(),
            change_history.clone(),
        );

        let indicator = use_cases
            .create(CreateCustomIndicatorCommand {
                name: " sample ".into(),
                strategy_id: Some(strategy_id),
                code: "return {}".into(),
                input_schema: json!({ "type": "object" }),
                output_schema: json!({ "type": "object" }),
                description: None,
            })
            .await
            .expect("indicator creation succeeds");
        let begun = unit_of_work.begun.lock().await.clone();
        let committed = unit_of_work.committed.lock().await.clone();
        let repository_transactions = repository.transaction_ids.lock().await.clone();
        let strategy_transactions = strategy_existence.transaction_ids().await;
        let history = change_history.entries.lock().await.clone();
        let transaction_id = begun.first().copied().unwrap_or_default();

        assert_eq!(
            (
                indicator.name,
                begun.len(),
                committed,
                repository_transactions
                    .iter()
                    .map(|id| *id == transaction_id)
                    .collect::<Vec<_>>(),
                strategy_transactions
                    .iter()
                    .map(|id| *id == transaction_id)
                    .collect::<Vec<_>>(),
                history
                    .into_iter()
                    .map(|entry| (
                        entry.transaction_id == transaction_id,
                        entry.record.actor,
                        entry.record.target_kind,
                        entry.record.target_id,
                        entry.record.op,
                        entry.record.diff,
                        entry.record.summary,
                    ))
                    .collect::<Vec<_>>(),
            ),
            (
                "sample".to_string(),
                1,
                vec![transaction_id],
                vec![true],
                vec![true],
                vec![(
                    true,
                    Actor::Human,
                    TargetKind::CustomIndicator,
                    indicator.indicator_id,
                    Op::Create,
                    json!({ "name": "sample", "scope": "strategy", "strategy_id": strategy_id }),
                    None,
                )],
            ),
        );
    }
}
