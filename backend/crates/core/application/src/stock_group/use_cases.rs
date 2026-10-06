use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::change_history::{Actor, ChangeHistoryRecord, Op, SharedChangeHistoryPort, TargetKind};
use crate::persistence::PersistenceError;
use crate::unit_of_work::SharedUnitOfWork;

use super::error::StockGroupUseCaseError;
use super::repository::{GroupAxis, SharedStockGroupRepository, StockGroupRepositoryError};
use super::types::{
    CreateStockGroupCommand, NewStockGroup, StockGroup, StockGroupCodeLookup, StockGroupMembership,
    UpdateStockGroupCommand,
};

#[derive(Clone)]
pub struct StockGroupUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedStockGroupRepository,
    change_history: SharedChangeHistoryPort,
}

impl StockGroupUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedStockGroupRepository,
        change_history: SharedChangeHistoryPort,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            change_history,
        }
    }

    pub async fn create(
        &self,
        command: CreateStockGroupCommand,
    ) -> Result<StockGroup, StockGroupUseCaseError> {
        let axis_key = validate_key("axis_key", &command.axis_key)?;
        let group_key = validate_key("group_key", &command.key)?;
        let name = validate_name(&command.name)?;
        let transaction = self.unit_of_work.begin().await?;
        let axis = self.axis(&transaction, &axis_key).await?;
        ensure_agent_managed(&axis)?;
        if self
            .repository
            .find_group(&transaction, axis.id, &axis.key, &group_key)
            .await?
            .is_some()
        {
            return Err(StockGroupUseCaseError::Validation(format!(
                "stock group {axis_key}/{group_key} already exists"
            )));
        }

        let group = self
            .repository
            .insert_group(
                &transaction,
                NewStockGroup {
                    id: Uuid::new_v4(),
                    axis_id: axis.id,
                    axis_key: axis_key.clone(),
                    key: group_key.clone(),
                    name,
                    description: command.description,
                },
            )
            .await
            .map_err(|error| match error {
                StockGroupRepositoryError::Database(PersistenceError::Conflict(_)) => {
                    StockGroupUseCaseError::Validation(format!(
                        "stock group {axis_key}/{group_key} already exists"
                    ))
                }
                error => error.into(),
            })?;
        self.record_history(
            &transaction,
            &group,
            Op::Create,
            json!({
                "axis_key": group.axis_key,
                "group_key": group.key,
                "name": group.name,
                "description": group.description,
            }),
        )
        .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(group)
    }

    pub async fn update(
        &self,
        command: UpdateStockGroupCommand,
    ) -> Result<StockGroup, StockGroupUseCaseError> {
        let axis_key = validate_key("axis_key", &command.axis_key)?;
        let group_key = validate_key("group_key", &command.key)?;
        let transaction = self.unit_of_work.begin().await?;
        let axis = self.axis(&transaction, &axis_key).await?;
        ensure_agent_managed(&axis)?;
        let current = self.group(&transaction, &axis, &group_key).await?;
        let mut updated = current.clone();
        let mut diff = Map::new();

        if let Some(name) = command.name {
            let name = validate_name(&name)?;
            if name != current.name {
                diff.insert("name".into(), json!({ "from": current.name, "to": name }));
                updated.name = name;
            }
        }
        if let Some(description) = command.description
            && description != current.description
        {
            diff.insert(
                "description".into(),
                json!({ "from": current.description, "to": description }),
            );
            updated.description = description;
        }

        if !diff.is_empty() {
            updated = self.repository.update_group(&transaction, updated).await?;
            self.record_history(&transaction, &updated, Op::Update, Value::Object(diff))
                .await?;
        }
        self.unit_of_work.commit(transaction).await?;
        Ok(updated)
    }

    pub async fn list_stock_ids(
        &self,
        axis_key: &str,
        group_key: &str,
    ) -> Result<Vec<String>, StockGroupUseCaseError> {
        let axis_key = validate_key("axis_key", axis_key)?;
        let group_key = validate_key("group_key", group_key)?;
        let transaction = self.unit_of_work.begin().await?;
        let axis = self.axis(&transaction, &axis_key).await?;
        let group = self.group(&transaction, &axis, &group_key).await?;
        let stock_ids = self
            .repository
            .list_stock_ids(&transaction, group.id)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(stock_ids)
    }

    pub async fn find_code_by_derive_from(
        &self,
        derive_from: &str,
        group_key: &str,
    ) -> Result<StockGroupCodeLookup, StockGroupUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let codes = self
            .repository
            .find_codes_by_derive_from(&transaction, derive_from, group_key)
            .await?;
        self.unit_of_work.commit(transaction).await?;

        if codes.is_empty() {
            return Ok(StockGroupCodeLookup::NotFound);
        }
        if codes.iter().any(Option::is_none) {
            return Ok(StockGroupCodeLookup::Missing);
        }

        let mut codes = codes.into_iter().flatten();
        let Some(code) = codes.next() else {
            return Ok(StockGroupCodeLookup::Ambiguous);
        };
        if codes.any(|other| other != code) {
            return Ok(StockGroupCodeLookup::Ambiguous);
        }
        Ok(StockGroupCodeLookup::Found(code))
    }

    pub async fn list_memberships(
        &self,
        stock_ids: &[String],
        axis_keys: &[String],
    ) -> Result<Vec<StockGroupMembership>, StockGroupUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let mut memberships = self
            .repository
            .list_memberships(&transaction, stock_ids, axis_keys)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        memberships.sort_by(|left, right| {
            (&left.axis_key, &left.group_key, &left.stock_id).cmp(&(
                &right.axis_key,
                &right.group_key,
                &right.stock_id,
            ))
        });
        Ok(memberships)
    }

    pub async fn add_stock(
        &self,
        axis_key: &str,
        group_key: &str,
        stock_id: &str,
    ) -> Result<bool, StockGroupUseCaseError> {
        self.change_membership(axis_key, group_key, stock_id, true)
            .await
    }

    pub async fn remove_stock(
        &self,
        axis_key: &str,
        group_key: &str,
        stock_id: &str,
    ) -> Result<bool, StockGroupUseCaseError> {
        self.change_membership(axis_key, group_key, stock_id, false)
            .await
    }

    async fn change_membership(
        &self,
        axis_key: &str,
        group_key: &str,
        stock_id: &str,
        add: bool,
    ) -> Result<bool, StockGroupUseCaseError> {
        let axis_key = validate_key("axis_key", axis_key)?;
        let group_key = validate_key("group_key", group_key)?;
        let stock_id = validate_key("stock_id", stock_id)?;
        let transaction = self.unit_of_work.begin().await?;
        let axis = self.axis(&transaction, &axis_key).await?;
        ensure_agent_managed(&axis)?;
        let group = self.group(&transaction, &axis, &group_key).await?;

        if add
            && !self
                .repository
                .stock_exists(&transaction, &stock_id)
                .await?
        {
            return Err(StockGroupUseCaseError::Validation(format!(
                "stock {stock_id} not found"
            )));
        }

        let changed = if add {
            self.repository
                .add_stock(&transaction, group.id, &stock_id)
                .await?
        } else {
            self.repository
                .remove_stock(&transaction, group.id, &stock_id)
                .await?
        };

        if changed {
            self.record_history(
                &transaction,
                &group,
                Op::Update,
                json!({
                    "stock_id": stock_id,
                    "membership": { "from": !add, "to": add },
                }),
            )
            .await?;
        }
        self.unit_of_work.commit(transaction).await?;
        Ok(changed)
    }

    async fn axis(
        &self,
        transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        axis_key: &str,
    ) -> Result<GroupAxis, StockGroupUseCaseError> {
        self.repository
            .find_axis_by_key(transaction, axis_key)
            .await?
            .ok_or_else(|| StockGroupUseCaseError::AxisNotFound(axis_key.to_owned()))
    }

    async fn group(
        &self,
        transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        axis: &GroupAxis,
        group_key: &str,
    ) -> Result<StockGroup, StockGroupUseCaseError> {
        self.repository
            .find_group(transaction, axis.id, &axis.key, group_key)
            .await?
            .ok_or_else(|| StockGroupUseCaseError::GroupNotFound {
                axis_key: axis.key.clone(),
                group_key: group_key.to_owned(),
            })
    }

    async fn record_history(
        &self,
        transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        group: &StockGroup,
        op: Op,
        diff: Value,
    ) -> Result<(), StockGroupUseCaseError> {
        self.change_history
            .record(
                transaction,
                ChangeHistoryRecord {
                    actor: Actor::Llm { label: "analyst" },
                    target_kind: TargetKind::StockGroup,
                    target_id: group.id,
                    op,
                    diff,
                    summary: None,
                },
            )
            .await?;
        Ok(())
    }
}

fn validate_key(field: &str, value: &str) -> Result<String, StockGroupUseCaseError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(StockGroupUseCaseError::Validation(format!(
            "{field} must not be empty"
        )));
    }
    Ok(value.to_owned())
}

fn validate_name(value: &str) -> Result<String, StockGroupUseCaseError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(StockGroupUseCaseError::Validation(
            "name must not be empty".into(),
        ));
    }
    Ok(value.to_owned())
}

fn ensure_agent_managed(axis: &GroupAxis) -> Result<(), StockGroupUseCaseError> {
    if axis.derive_from.is_some() {
        return Err(StockGroupUseCaseError::Validation(
            "stock groups on synchronized axes cannot be changed by MCP".into(),
        ));
    }
    Ok(())
}
