use chrono::Utc;
use core_application::dashboard::{
    Dashboard, DashboardData, DashboardRepository, DashboardRepositoryError,
};
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, QueryOrder};

use crate::DatabaseHandle;
use crate::entities::dashboard;
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresDashboardRepository {
    db: DatabaseHandle,
}

impl PostgresDashboardRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl DashboardRepository for PostgresDashboardRepository {
    async fn list(&self) -> Result<Vec<Dashboard>, DashboardRepositoryError> {
        let models = dashboard::Entity::find()
            .order_by_asc(dashboard::Column::Key)
            .all(&self.db)
            .await
            .map_err(persistence_error)?;
        Ok(models.into_iter().map(from_model).collect())
    }

    async fn get(&self, key: &str) -> Result<Option<Dashboard>, DashboardRepositoryError> {
        let model = dashboard::Entity::find_by_id(key.to_owned())
            .one(&self.db)
            .await
            .map_err(persistence_error)?;
        Ok(model.map(from_model))
    }

    async fn create(&self, data: DashboardData) -> Result<Dashboard, DashboardRepositoryError> {
        let key = data.key;
        let model = dashboard::Entity::insert(dashboard::ActiveModel {
            key: Set(key.clone()),
            config: Set(data.config),
            created_at: NotSet,
            updated_at: NotSet,
        })
        .exec_with_returning(&self.db)
        .await
        .map_err(|error| map_write_error(error, &key))?;

        Ok(from_model(model))
    }

    async fn update(
        &self,
        key: &str,
        config: String,
    ) -> Result<Option<Dashboard>, DashboardRepositoryError> {
        let Some(model) = dashboard::Entity::find_by_id(key.to_owned())
            .one(&self.db)
            .await
            .map_err(persistence_error)?
        else {
            return Ok(None);
        };

        let mut active_model = model.into_active_model();
        active_model.config = Set(config);
        active_model.updated_at = Set(Utc::now().fixed_offset());
        let updated = match active_model.update(&self.db).await {
            Ok(model) => model,
            Err(sea_orm::DbErr::RecordNotUpdated) => return Ok(None),
            Err(error) => return Err(persistence_error(error).into()),
        };

        Ok(Some(from_model(updated)))
    }

    async fn delete(&self, key: &str) -> Result<bool, DashboardRepositoryError> {
        dashboard::Entity::delete_by_id(key.to_owned())
            .exec(&self.db)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(persistence_error)
            .map_err(Into::into)
    }
}

fn from_model(model: dashboard::Model) -> Dashboard {
    Dashboard {
        key: model.key,
        config: model.config,
        created_at: model.created_at,
        updated_at: model.updated_at,
    }
}

fn map_write_error(error: sea_orm::DbErr, key: &str) -> DashboardRepositoryError {
    match persistence_error(error) {
        core_application::persistence::PersistenceError::Conflict(_) => {
            DashboardRepositoryError::DuplicateKey(key.to_owned())
        }
        error => DashboardRepositoryError::Persistence(error),
    }
}
