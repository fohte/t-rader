use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::indicator_observation::{
    IndicatorObservationMetadata, IndicatorObservationQuery, IndicatorObservationRepository,
    IndicatorObservationRepositoryError,
};
use core_domain::IndicatorObservation;
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

use crate::DatabaseHandle;
use crate::entities::{indicator, indicator_observation};
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresIndicatorObservationRepository {
    db: DatabaseHandle,
}

impl PostgresIndicatorObservationRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl IndicatorObservationRepository for PostgresIndicatorObservationRepository {
    async fn ensure_indicator(
        &self,
        metadata: IndicatorObservationMetadata,
    ) -> Result<(), IndicatorObservationRepositoryError> {
        indicator::Entity::insert(indicator::ActiveModel {
            id: Set(metadata.indicator_id),
            name: Set(metadata.name),
            kind: Set(metadata.kind),
        })
        .on_conflict(
            OnConflict::column(indicator::Column::Id)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(&self.db)
        .await
        .map(|_| ())
        .map_err(repository_error)
    }

    async fn find_latest_date(
        &self,
        indicator_id: &str,
    ) -> Result<Option<NaiveDate>, IndicatorObservationRepositoryError> {
        indicator_observation::Entity::find()
            .filter(indicator_observation::Column::IndicatorId.eq(indicator_id))
            .order_by_desc(indicator_observation::Column::Date)
            .one(&self.db)
            .await
            .map(|row| row.map(|row| row.date))
            .map_err(repository_error)
    }

    async fn upsert_observations(
        &self,
        indicator_id: &str,
        observations: Vec<IndicatorObservation>,
    ) -> Result<usize, IndicatorObservationRepositoryError> {
        if observations.is_empty() {
            return Ok(0);
        }
        let count = observations.len();
        let models: Vec<indicator_observation::ActiveModel> = observations
            .into_iter()
            .map(|observation| indicator_observation::ActiveModel {
                indicator_id: Set(indicator_id.to_string()),
                date: Set(observation.date),
                value: Set(observation.value),
            })
            .collect();

        indicator_observation::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([
                    indicator_observation::Column::IndicatorId,
                    indicator_observation::Column::Date,
                ])
                .update_column(indicator_observation::Column::Value)
                .to_owned(),
            )
            .exec_without_returning(&self.db)
            .await
            .map_err(repository_error)?;

        Ok(count)
    }

    async fn find_observations(
        &self,
        query: IndicatorObservationQuery,
    ) -> Result<Vec<IndicatorObservation>, IndicatorObservationRepositoryError> {
        indicator_observation::Entity::find()
            .filter(indicator_observation::Column::IndicatorId.eq(query.indicator_id))
            .filter(indicator_observation::Column::Date.gte(query.from))
            .filter(indicator_observation::Column::Date.lte(query.to))
            .order_by_asc(indicator_observation::Column::Date)
            .all(&self.db)
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|row| IndicatorObservation {
                        date: row.date,
                        value: row.value,
                    })
                    .collect()
            })
            .map_err(repository_error)
    }
}

fn repository_error(error: sea_orm::DbErr) -> IndicatorObservationRepositoryError {
    persistence_error(error).into()
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::indicator_observation::{
        IndicatorObservationMetadata, IndicatorObservationQuery, IndicatorObservationRepository,
    };
    use core_domain::IndicatorObservation;
    use rust_decimal::Decimal;
    use sea_orm::EntityTrait;

    use super::PostgresIndicatorObservationRepository;
    use crate::entities::indicator;

    fn date(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, day).expect("valid date")
    }

    fn metadata(name: &str, kind: &str) -> IndicatorObservationMetadata {
        IndicatorObservationMetadata {
            indicator_id: "INDICATOR_TEST".to_string(),
            name: name.to_string(),
            kind: kind.to_string(),
        }
    }

    fn observation(day: u32, value: i64) -> IndicatorObservation {
        IndicatorObservation {
            date: date(day),
            value: Decimal::from(value),
        }
    }

    async fn seeded_repository(
        db: crate::DatabaseHandle,
    ) -> PostgresIndicatorObservationRepository {
        let repository = PostgresIndicatorObservationRepository::new(db);
        repository
            .ensure_indicator(metadata("テスト指標", "index"))
            .await
            .expect("create indicator");
        repository
            .upsert_observations(
                "INDICATOR_TEST",
                vec![observation(1, 10), observation(3, 30), observation(10, 100)],
            )
            .await
            .expect("insert observations");
        repository
    }

    #[backend_test_macros::database_test]
    async fn ensures_indicator_without_replacing_existing_metadata(db: crate::DatabaseHandle) {
        let repository = PostgresIndicatorObservationRepository::new(db.clone());
        repository
            .ensure_indicator(metadata("既存名", "custom"))
            .await
            .expect("create indicator");
        repository
            .ensure_indicator(metadata("新しい名前", "index"))
            .await
            .expect("keep existing indicator");

        let actual = indicator::Entity::find_by_id("INDICATOR_TEST".to_string())
            .one(&db)
            .await
            .expect("query succeeds");

        assert_eq!(
            actual,
            Some(indicator::Model {
                id: "INDICATOR_TEST".to_string(),
                name: "既存名".to_string(),
                kind: "custom".to_string(),
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn upsert_replaces_observation_for_the_same_date(db: crate::DatabaseHandle) {
        let repository = seeded_repository(db).await;
        let updated = repository
            .upsert_observations("INDICATOR_TEST", vec![observation(3, 35)])
            .await
            .expect("update observation");
        let observations = repository
            .find_observations(IndicatorObservationQuery {
                indicator_id: "INDICATOR_TEST".to_string(),
                from: date(3),
                to: date(3),
            })
            .await
            .expect("read observations");

        assert_eq!((updated, observations), (1, vec![observation(3, 35)]),);
    }

    #[backend_test_macros::database_test]
    async fn find_latest_date_returns_newest_observation_date(db: crate::DatabaseHandle) {
        let repository = seeded_repository(db).await;
        let latest = repository
            .find_latest_date("INDICATOR_TEST")
            .await
            .expect("find latest date");

        assert_eq!(latest, Some(date(10)));
    }

    #[backend_test_macros::database_test]
    async fn find_observations_includes_range_boundaries_in_ascending_order(
        db: crate::DatabaseHandle,
    ) {
        let repository = seeded_repository(db).await;
        let observations = repository
            .find_observations(IndicatorObservationQuery {
                indicator_id: "INDICATOR_TEST".to_string(),
                from: date(1),
                to: date(10),
            })
            .await
            .expect("read observations");

        assert_eq!(
            observations,
            vec![observation(1, 10), observation(3, 30), observation(10, 100)],
        );
    }
}
