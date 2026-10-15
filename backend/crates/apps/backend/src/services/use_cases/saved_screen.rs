use std::sync::Arc;

use core_application::saved_screen::SavedScreenUseCases;
use gateway_postgres::PostgresSavedScreenRepository;

use super::UseCases;

impl UseCases {
    pub fn saved_screens(&self) -> SavedScreenUseCases {
        SavedScreenUseCases::new(Arc::new(PostgresSavedScreenRepository::new(
            self.db.clone(),
        )))
    }
}
