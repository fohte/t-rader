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
mod m20260912_103004_add_strategy_task_as_of;
mod m20260912_103436_add_strategy_task_step_evidence;
mod m20260912_105611_add_checkpoint;
mod m20260912_105634_add_trade_note_and_note_hypothesis_links;
mod m20260912_105636_add_news_strategy_link_seq;
mod m20260912_130617_add_hypothesis_proposal;
mod m20260912_161307_add_jquants_plan_setting;
mod m20260912_162403_add_mcp_tool_call_count;
mod m20260913_072329_add_margin_tables;
mod m20260913_072507_add_edinet_holdings;
mod m20260913_072524_add_jquants_fin_summary;
mod m20260913_073037_add_short_selling_data;
mod m20260913_081737_add_ref_term;
mod m20260913_082645_add_custom_indicator_to_change_history_target_kind;
mod m20260913_091652_add_jquants_fin_summary_code_prefix_index;
mod m20260913_122738_add_indicator_observation;
mod m20260913_122746_add_stock_product_category;
mod m20260913_122949_add_jquants_daily_bars_ingested_date;
mod m20260913_125042_add_prediction;
mod m20260913_133130_add_margin_code_prefix_indexes;
mod m20260913_142709_add_jquants_earnings_date;
mod m20260915_120353_add_annotation_execution_tracking;
mod m20260915_165800_add_strategy_task_auto_resumed_at;
mod m20260915_170728_add_prediction_grade;
mod m20260915_174024_add_calc_date_to_short_sale_report_pk;
mod m20260919_052834_add_strategy_task_as_of_comment;
mod m20260922_122232_add_jquants_valuation;
mod m20260924_171744_note_kind;
mod m20260924_171836_note_version;
mod m20260925_035409_remove_watchlist;
mod m20260925_043733_note_version_review;
mod m20260925_043735_note_links_and_trade_note_versions;
mod m20260925_075026_drop_news_strategy_link;
mod m20260925_093011_drop_strategy_interest;
mod m20260925_115400_add_note_kind_fk_and_nonblank_checks;
mod m20260925_122811_rename_valuation_tables;
mod m20260925_122813_edinet_holdings_port;
mod m20260926_063200_remove_hypothesis_tables;

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
            Box::new(m20260912_103004_add_strategy_task_as_of::Migration),
            Box::new(m20260912_103436_add_strategy_task_step_evidence::Migration),
            Box::new(m20260912_105611_add_checkpoint::Migration),
            Box::new(m20260912_105634_add_trade_note_and_note_hypothesis_links::Migration),
            Box::new(m20260912_105636_add_news_strategy_link_seq::Migration),
            Box::new(m20260912_130617_add_hypothesis_proposal::Migration),
            Box::new(m20260912_161307_add_jquants_plan_setting::Migration),
            Box::new(m20260912_162403_add_mcp_tool_call_count::Migration),
            Box::new(m20260913_072329_add_margin_tables::Migration),
            Box::new(m20260913_072507_add_edinet_holdings::Migration),
            Box::new(m20260913_072524_add_jquants_fin_summary::Migration),
            Box::new(m20260913_073037_add_short_selling_data::Migration),
            Box::new(m20260913_081737_add_ref_term::Migration),
            Box::new(
                m20260913_082645_add_custom_indicator_to_change_history_target_kind::Migration,
            ),
            Box::new(m20260913_091652_add_jquants_fin_summary_code_prefix_index::Migration),
            Box::new(m20260913_122738_add_indicator_observation::Migration),
            Box::new(m20260913_122746_add_stock_product_category::Migration),
            Box::new(m20260913_122949_add_jquants_daily_bars_ingested_date::Migration),
            Box::new(m20260913_125042_add_prediction::Migration),
            Box::new(m20260913_133130_add_margin_code_prefix_indexes::Migration),
            Box::new(m20260913_142709_add_jquants_earnings_date::Migration),
            Box::new(m20260915_120353_add_annotation_execution_tracking::Migration),
            Box::new(m20260915_165800_add_strategy_task_auto_resumed_at::Migration),
            Box::new(m20260915_170728_add_prediction_grade::Migration),
            Box::new(m20260915_174024_add_calc_date_to_short_sale_report_pk::Migration),
            Box::new(m20260919_052834_add_strategy_task_as_of_comment::Migration),
            Box::new(m20260922_122232_add_jquants_valuation::Migration),
            Box::new(m20260924_171744_note_kind::Migration),
            Box::new(m20260924_171836_note_version::Migration),
            Box::new(m20260925_035409_remove_watchlist::Migration),
            Box::new(m20260925_043733_note_version_review::Migration),
            Box::new(m20260925_043735_note_links_and_trade_note_versions::Migration),
            Box::new(m20260925_075026_drop_news_strategy_link::Migration),
            Box::new(m20260925_093011_drop_strategy_interest::Migration),
            Box::new(m20260925_115400_add_note_kind_fk_and_nonblank_checks::Migration),
            Box::new(m20260925_122811_rename_valuation_tables::Migration),
            Box::new(m20260925_122813_edinet_holdings_port::Migration),
            Box::new(m20260926_063200_remove_hypothesis_tables::Migration),
        ]
    }
}
