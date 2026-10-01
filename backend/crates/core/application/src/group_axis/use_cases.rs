use crate::persistence::PersistenceError;
use crate::unit_of_work::SharedUnitOfWork;

use super::error::GroupAxisUseCaseError;
use super::repository::{GroupAxisRepositoryError, SharedGroupAxisRepository};
use super::types::{CreateGroupAxisCommand, GroupAxis, NewGroupAxis, UpdateGroupAxisCommand};

#[derive(Clone)]
pub struct GroupAxisUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedGroupAxisRepository,
}

impl GroupAxisUseCases {
    pub fn new(unit_of_work: SharedUnitOfWork, repository: SharedGroupAxisRepository) -> Self {
        Self {
            unit_of_work,
            repository,
        }
    }

    pub async fn list(&self) -> Result<Vec<GroupAxis>, GroupAxisUseCaseError> {
        self.repository.list().await.map_err(Into::into)
    }

    pub async fn get(&self, key: &str) -> Result<GroupAxis, GroupAxisUseCaseError> {
        let key = validate_key(key)?;
        let transaction = self.unit_of_work.begin().await?;
        let axis = self.repository.find_by_key(&transaction, &key).await?;
        let axis = axis.ok_or_else(|| {
            GroupAxisUseCaseError::NotFound(format!("group axis {key} not found"))
        })?;
        self.unit_of_work.commit(transaction).await?;
        Ok(axis)
    }

    pub async fn create(
        &self,
        command: CreateGroupAxisCommand,
    ) -> Result<GroupAxis, GroupAxisUseCaseError> {
        let key = validate_key(&command.key)?;
        let name = validate_name(&command.name)?;
        let transaction = self.unit_of_work.begin().await?;
        let created = match self
            .repository
            .insert(
                &transaction,
                NewGroupAxis {
                    key: key.clone(),
                    name,
                    description: command.description,
                    sync_source: command.sync_source,
                },
            )
            .await
        {
            Ok(axis) => axis,
            Err(GroupAxisRepositoryError::Database(PersistenceError::Conflict(_))) => {
                return Err(GroupAxisUseCaseError::Conflict(format!(
                    "group axis {key} already exists"
                )));
            }
            Err(error) => return Err(error.into()),
        };
        self.unit_of_work.commit(transaction).await?;
        Ok(created)
    }

    pub async fn update(
        &self,
        key: &str,
        command: UpdateGroupAxisCommand,
    ) -> Result<GroupAxis, GroupAxisUseCaseError> {
        let key = validate_key(key)?;
        let transaction = self.unit_of_work.begin().await?;
        let current = self
            .repository
            .find_by_key(&transaction, &key)
            .await?
            .ok_or_else(|| {
                GroupAxisUseCaseError::NotFound(format!("group axis {key} not found"))
            })?;
        let mut updated = current.clone();
        if let Some(name) = command.name {
            updated.name = validate_name(&name)?;
        }
        if let Some(description) = command.description {
            updated.description = description;
        }
        if let Some(sync_source) = command.sync_source {
            updated.sync_source = sync_source;
        }
        if updated == current {
            self.unit_of_work.commit(transaction).await?;
            return Ok(current);
        }
        let updated = self.repository.update(&transaction, updated).await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(updated)
    }

    pub async fn delete(&self, key: &str) -> Result<(), GroupAxisUseCaseError> {
        let key = validate_key(key)?;
        let transaction = self.unit_of_work.begin().await?;
        self.repository
            .find_by_key(&transaction, &key)
            .await?
            .ok_or_else(|| {
                GroupAxisUseCaseError::NotFound(format!("group axis {key} not found"))
            })?;
        match self.repository.delete(&transaction, &key).await {
            Ok(true) => {}
            Ok(false) => {
                return Err(GroupAxisUseCaseError::NotFound(format!(
                    "group axis {key} not found"
                )));
            }
            Err(GroupAxisRepositoryError::Database(PersistenceError::MissingReference(_))) => {
                return Err(GroupAxisUseCaseError::Conflict(format!(
                    "group axis {key} cannot be deleted while it contains groups"
                )));
            }
            Err(error) => return Err(error.into()),
        }
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }
}

fn validate_key(key: &str) -> Result<String, GroupAxisUseCaseError> {
    if key.trim().is_empty() || key != key.trim() || key.contains('/') {
        return Err(GroupAxisUseCaseError::Validation(
            "key must not be empty, contain '/', or have surrounding whitespace".into(),
        ));
    }
    Ok(key.to_string())
}

fn validate_name(name: &str) -> Result<String, GroupAxisUseCaseError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(GroupAxisUseCaseError::Validation(
            "name must not be empty".into(),
        ));
    }
    Ok(name.to_string())
}
