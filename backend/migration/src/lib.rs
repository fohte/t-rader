pub use sea_orm_migration::prelude::*;

mod m20260215_092115_initial_schema;
mod m20260601_142149_redesign_schema;
mod m20260614_110009_add_mcp_session_state;
mod m20260615_132751_strategy_task;
mod m20260618_154453_add_strategy_agent_columns;
mod m20260625_020801_trigger;
mod m20260625_103524_custom_indicator;

pub struct Migrator;

impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260215_092115_initial_schema::Migration),
            Box::new(m20260601_142149_redesign_schema::Migration),
            Box::new(m20260614_110009_add_mcp_session_state::Migration),
            Box::new(m20260615_132751_strategy_task::Migration),
            Box::new(m20260618_154453_add_strategy_agent_columns::Migration),
            Box::new(m20260625_020801_trigger::Migration),
            Box::new(m20260625_103524_custom_indicator::Migration),
        ]
    }
}
