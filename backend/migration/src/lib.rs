pub use sea_orm_migration::prelude::*;

mod m20260215_092115_initial_schema;
mod m20260601_142149_redesign_schema;
mod m20260614_110009_add_mcp_session_state;

pub struct Migrator;

impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260215_092115_initial_schema::Migration),
            Box::new(m20260601_142149_redesign_schema::Migration),
            Box::new(m20260614_110009_add_mcp_session_state::Migration),
        ]
    }
}
