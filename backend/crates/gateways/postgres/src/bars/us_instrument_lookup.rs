use core_application::bars::BarsRepositoryError;
use core_domain::instrument::Market;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::entities::instruments;

use super::{PostgresBarsRepository, repository_error};

impl PostgresBarsRepository {
    pub(super) async fn us_instrument_ids(
        &self,
        instrument_ids: &[String],
    ) -> Result<Vec<String>, BarsRepositoryError> {
        if instrument_ids.is_empty() {
            return Ok(Vec::new());
        }

        instruments::Entity::find()
            .filter(instruments::Column::Id.is_in(instrument_ids.to_vec()))
            .filter(instruments::Column::Market.eq(Market::Us.to_string()))
            .all(&self.db)
            .await
            .map(|instruments| {
                instruments
                    .into_iter()
                    .map(|instrument| instrument.id)
                    .collect()
            })
            .map_err(repository_error)
    }
}
