use async_trait::async_trait;
use core_application::account_risk_policy::{
    AccountRiskPolicyRepository, AccountRiskPolicyRepositoryError,
};
use sea_orm::ActiveValue::Set;
use sea_orm::EntityTrait;
use sea_orm::sea_query::OnConflict;
use serde_json::Value;

use crate::DatabaseHandle;
use crate::entities::account_risk_policy;
use crate::persistence::persistence_error;

const SINGLETON_ID: i16 = 1;

#[derive(Clone)]
pub struct PostgresAccountRiskPolicyRepository {
    db: DatabaseHandle,
}

impl PostgresAccountRiskPolicyRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AccountRiskPolicyRepository for PostgresAccountRiskPolicyRepository {
    async fn find_current(&self) -> Result<Option<Value>, AccountRiskPolicyRepositoryError> {
        account_risk_policy::Entity::find_by_id(SINGLETON_ID)
            .one(&self.db)
            .await
            .map(|row| row.map(|model| model.risk_policy))
            .map_err(persistence_error)
            .map_err(Into::into)
    }

    async fn save(&self, risk_policy: Value) -> Result<Value, AccountRiskPolicyRepositoryError> {
        let previous = self.find_current().await?;
        let model = account_risk_policy::ActiveModel {
            id: Set(SINGLETON_ID),
            risk_policy: Set(risk_policy),
            updated_at: Set(chrono::Utc::now().fixed_offset()),
        };
        let saved = account_risk_policy::Entity::insert(model)
            .on_conflict(
                OnConflict::column(account_risk_policy::Column::Id)
                    .update_columns([
                        account_risk_policy::Column::RiskPolicy,
                        account_risk_policy::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec_with_returning(&self.db)
            .await
            .map_err(persistence_error)?;

        tracing::info!(
            from = ?previous,
            to = ?saved.risk_policy,
            "updated account_risk_policy",
        );
        Ok(saved.risk_policy)
    }
}

#[cfg(test)]
mod tests {
    use core_application::account_risk_policy::AccountRiskPolicyRepository;
    use sea_orm::EntityTrait;
    use serde_json::json;

    use super::PostgresAccountRiskPolicyRepository;
    use crate::DatabaseHandle;
    use crate::entities::account_risk_policy;

    #[backend_test_macros::database_test]
    async fn find_current_returns_none_when_row_is_missing(db: DatabaseHandle) {
        let repository = PostgresAccountRiskPolicyRepository::new(db);

        assert_eq!(repository.find_current().await.expect("read policy"), None);
    }

    #[backend_test_macros::database_test]
    async fn save_upserts_the_singleton_row(db: DatabaseHandle) {
        let repository = PostgresAccountRiskPolicyRepository::new(db.clone());
        let first = json!({ "max_group_ratios": [] });
        let latest = json!({ "max_group_ratios": [] });

        let created = repository.save(first).await.expect("create policy");
        let updated = repository
            .save(latest.clone())
            .await
            .expect("update policy");
        let rows = account_risk_policy::Entity::find()
            .all(&db)
            .await
            .expect("list policy rows");
        let saved_row = rows.first().map(|row| (row.id, row.risk_policy.clone()));

        assert_eq!(
            (created, updated, rows.len(), saved_row),
            (
                json!({ "max_group_ratios": [] }),
                latest.clone(),
                1,
                Some((1, latest)),
            ),
        );
    }
}
