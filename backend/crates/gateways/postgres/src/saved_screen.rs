use async_trait::async_trait;
use chrono::Utc;
use core_application::saved_screen::{
    SavedScreen, SavedScreenData, SavedScreenRepository, SavedScreenRepositoryError,
};
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, QueryOrder};
use serde::Serialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::saved_screen;
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresSavedScreenRepository {
    db: DatabaseHandle,
}

impl PostgresSavedScreenRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl SavedScreenRepository for PostgresSavedScreenRepository {
    async fn list(&self) -> Result<Vec<SavedScreen>, SavedScreenRepositoryError> {
        saved_screen::Entity::find()
            .order_by_asc(saved_screen::Column::Name)
            .all(&self.db)
            .await
            .map_err(persistence_error)?
            .into_iter()
            .map(from_model)
            .collect()
    }

    async fn create(
        &self,
        data: SavedScreenData,
    ) -> Result<SavedScreen, SavedScreenRepositoryError> {
        let name = data.name;
        let model = saved_screen::Entity::insert(saved_screen::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(name.clone()),
            filters: Set(to_json(data.filters)?),
            sort: Set(to_json(data.sort)?),
            created_at: NotSet,
            updated_at: NotSet,
        })
        .exec_with_returning(&self.db)
        .await
        .map_err(|error| map_write_error(error, &name))?;

        from_model(model)
    }

    async fn update(
        &self,
        id: Uuid,
        data: SavedScreenData,
    ) -> Result<Option<SavedScreen>, SavedScreenRepositoryError> {
        let Some(model) = saved_screen::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(persistence_error)?
        else {
            return Ok(None);
        };

        let name = data.name;
        let mut active_model = model.into_active_model();
        active_model.name = Set(name.clone());
        active_model.filters = Set(to_json(data.filters)?);
        active_model.sort = Set(to_json(data.sort)?);
        active_model.updated_at = Set(Utc::now().fixed_offset());
        let updated = match active_model.update(&self.db).await {
            Ok(model) => model,
            Err(sea_orm::DbErr::RecordNotUpdated) => return Ok(None),
            Err(error) => return Err(map_write_error(error, &name)),
        };

        from_model(updated).map(Some)
    }

    async fn delete(&self, id: Uuid) -> Result<bool, SavedScreenRepositoryError> {
        saved_screen::Entity::delete_by_id(id)
            .exec(&self.db)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(persistence_error)
            .map_err(Into::into)
    }
}

fn to_json(value: impl Serialize) -> Result<serde_json::Value, SavedScreenRepositoryError> {
    serde_json::to_value(value)
        .map_err(|error| SavedScreenRepositoryError::InvalidData(error.to_string()))
}

fn from_json<T: DeserializeOwned>(
    value: serde_json::Value,
) -> Result<T, SavedScreenRepositoryError> {
    serde_json::from_value(value)
        .map_err(|error| SavedScreenRepositoryError::InvalidData(error.to_string()))
}

fn from_model(model: saved_screen::Model) -> Result<SavedScreen, SavedScreenRepositoryError> {
    Ok(SavedScreen {
        id: model.id,
        name: model.name,
        filters: from_json(model.filters)?,
        sort: from_json(model.sort)?,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}

fn map_write_error(error: sea_orm::DbErr, name: &str) -> SavedScreenRepositoryError {
    match persistence_error(error) {
        core_application::persistence::PersistenceError::Conflict(_) => {
            SavedScreenRepositoryError::DuplicateName(name.to_owned())
        }
        error => SavedScreenRepositoryError::Persistence(error),
    }
}
