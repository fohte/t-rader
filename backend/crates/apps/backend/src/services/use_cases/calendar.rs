use std::sync::Arc;

use async_trait::async_trait;
use core_application::calendar::{
    read_use_cases::{
        CalendarEventReadUseCases, CalendarEventTargetSource, CalendarEventTargetSourceError,
    },
    use_cases::CalendarEventUseCases,
};
use core_application::strategy_earnings_target::StrategyEarningsTargetUseCases;
use core_application::strategy_scope::StrategyScope;
use core_application::stock_group::StockGroupUseCases;
use gateway_postgres::{PostgresCalendarEventRepository, PostgresUnitOfWork};
use std::collections::HashSet;

use super::UseCases;

impl UseCases {
    pub fn calendar_events(&self) -> CalendarEventUseCases {
        CalendarEventUseCases::new(
            Arc::new(PostgresCalendarEventRepository::new(self.db.clone())),
            Arc::new(PostgresUnitOfWork::new(self.db.clone())),
        )
    }

    pub fn calendar_event_reads(&self) -> CalendarEventReadUseCases {
        CalendarEventReadUseCases::new(
            Arc::new(PostgresCalendarEventRepository::new(self.db.clone())),
            Arc::new(CalendarEventTargetSourceAdapter {
                strategy_earnings_targets: self.strategy_earnings_targets(),
                stock_groups: self.stock_groups(),
            }),
        )
    }
}

struct CalendarEventTargetSourceAdapter {
    strategy_earnings_targets: StrategyEarningsTargetUseCases,
    stock_groups: StockGroupUseCases,
}

#[async_trait]
impl CalendarEventTargetSource for CalendarEventTargetSourceAdapter {
    async fn list_stock_ids(
        &self,
        scope: StrategyScope,
    ) -> Result<Vec<String>, CalendarEventTargetSourceError> {
        let targets = self
            .strategy_earnings_targets
            .list(scope)
            .await
            .map_err(|error| CalendarEventTargetSourceError::Failed(error.to_string()))?;
        let mut stock_ids = HashSet::new();

        for target in targets {
            match target.ref_kind.as_str() {
                "stock" => {
                    stock_ids.insert(target.ref_id);
                }
                "group" => {
                    let Some((axis_key, group_key)) = target.ref_id.split_once('/') else {
                        return Err(CalendarEventTargetSourceError::Failed(format!(
                            "invalid group reference: {}",
                            target.ref_id
                        )));
                    };
                    stock_ids.extend(
                        self.stock_groups
                            .list_stock_ids(axis_key, group_key)
                            .await
                            .map_err(|error| {
                                CalendarEventTargetSourceError::Failed(error.to_string())
                            })?,
                    );
                }
                ref_kind => {
                    return Err(CalendarEventTargetSourceError::Failed(format!(
                        "unknown earnings target kind: {ref_kind}"
                    )));
                }
            }
        }

        Ok(stock_ids.into_iter().collect())
    }
}
