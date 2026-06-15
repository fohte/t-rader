pub use sea_orm_migration::prelude::*;

mod m20260215_092115_initial_schema;
mod m20260601_142149_redesign_schema;
mod m20260615_132751_strategy_task;

pub struct Migrator;

impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260215_092115_initial_schema::Migration),
            Box::new(m20260601_142149_redesign_schema::Migration),
            Box::new(m20260615_132751_strategy_task::Migration),
        ]
    }
}
