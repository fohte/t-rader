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
mod m20260814_183203_add_comment_resolved;
mod m20260814_185221_add_strategy_task_steps;
mod m20260815_060751_add_comment_anchor;
mod m20260831_165304_drop_annotation_target_kind_check;
mod m20260903_162322_add_note_execution_id;
mod m20260906_155928_drop_mcp_session_state;
mod m20260909_125717_make_note_annotation_strategy_id_nullable;
mod m20260909_140059_add_strategy_investable_amount;
mod m20260909_163939_add_risk_policy;
mod m20260909_165140_make_strategy_interest_strategy_id_nullable;
mod m20260909_165900_make_hypothesis_trigger_strategy_id_nullable;
mod m20260910_034756_add_agent_config;
mod m20260910_144115_add_strategy_task_purpose;
mod m20260910_144144_add_strategy_interest_status;
mod m20260911_005614_drop_strategy_agent_columns;
mod m20260911_153846_add_strategy_task_step;
mod m20260912_022210_drop_strategy_risk_policy;
mod m20260912_103436_add_strategy_task_step_evidence;
mod m20260912_105611_add_checkpoint;
mod m20260912_105636_add_news_strategy_link_seq;

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
            Box::new(m20260814_183203_add_comment_resolved::Migration),
            Box::new(m20260814_185221_add_strategy_task_steps::Migration),
            Box::new(m20260815_060751_add_comment_anchor::Migration),
            Box::new(m20260831_165304_drop_annotation_target_kind_check::Migration),
            Box::new(m20260903_162322_add_note_execution_id::Migration),
            Box::new(m20260906_155928_drop_mcp_session_state::Migration),
            Box::new(m20260909_125717_make_note_annotation_strategy_id_nullable::Migration),
            Box::new(m20260909_140059_add_strategy_investable_amount::Migration),
            Box::new(m20260909_163939_add_risk_policy::Migration),
            Box::new(m20260909_165140_make_strategy_interest_strategy_id_nullable::Migration),
            Box::new(m20260909_165900_make_hypothesis_trigger_strategy_id_nullable::Migration),
            Box::new(m20260910_034756_add_agent_config::Migration),
            Box::new(m20260910_144115_add_strategy_task_purpose::Migration),
            Box::new(m20260910_144144_add_strategy_interest_status::Migration),
            Box::new(m20260911_005614_drop_strategy_agent_columns::Migration),
            Box::new(m20260911_153846_add_strategy_task_step::Migration),
            Box::new(m20260912_022210_drop_strategy_risk_policy::Migration),
            Box::new(m20260912_103436_add_strategy_task_step_evidence::Migration),
            Box::new(m20260912_105611_add_checkpoint::Migration),
            Box::new(m20260912_105636_add_news_strategy_link_seq::Migration),
        ]
    }
}
