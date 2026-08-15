pub use sea_orm_migration::prelude::*;

mod m20260215_092115_initial_schema;
mod m20260601_142149_redesign_schema;
mod m20260614_110009_add_mcp_session_state;
mod m20260615_132751_strategy_task;
mod m20260618_154453_add_strategy_agent_columns;
mod m20260625_020801_trigger;
mod m20260625_103524_custom_indicator;
mod m20260626_131137_news_aggregation;
mod m20260628_065359_hypothesis;
mod m20260628_111309_add_rss_feed;
mod m20260715_145104_strategy_task_a2a_migration;
mod m20260717_144924_remove_strategy_agent_status;
mod m20260810_114448_add_strategy_agent_graph;
mod m20260814_182703_add_note_graphs_json;
mod m20260814_185221_add_strategy_task_steps;

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
            Box::new(m20260626_131137_news_aggregation::Migration),
            Box::new(m20260628_065359_hypothesis::Migration),
            Box::new(m20260628_111309_add_rss_feed::Migration),
            Box::new(m20260715_145104_strategy_task_a2a_migration::Migration),
            Box::new(m20260717_144924_remove_strategy_agent_status::Migration),
            Box::new(m20260810_114448_add_strategy_agent_graph::Migration),
            Box::new(m20260814_182703_add_note_graphs_json::Migration),
            Box::new(m20260814_185221_add_strategy_task_steps::Migration),
        ]
    }
}
