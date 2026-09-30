use core_application::change_history::ChangeHistoryUseCases;

use super::UseCases;

impl UseCases {
    pub fn change_history(&self) -> ChangeHistoryUseCases {
        ChangeHistoryUseCases::new(self.change_history_query.clone())
    }
}
