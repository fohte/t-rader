use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::api_doc::ApiDoc;
use crate::handlers::{
    agent_config, agent_options, annotations, bars, comments, config, custom_indicators,
    group_axes, history, imports, ingest_status, note_kinds, note_links, note_predictions,
    note_versions, notes, refs, risk_policy, rss_feeds, strategies, tasks, trade_notes, trades,
    triggers,
};
use crate::state::FrontendApiState;

pub fn router() -> OpenApiRouter<FrontendApiState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(ingest_status::get_ingest_status))
        .routes(routes!(bars::list_bars))
        .routes(routes!(
            strategies::list_strategies,
            strategies::create_strategy
        ))
        .routes(routes!(
            strategies::get_strategy,
            strategies::update_strategy,
            strategies::delete_strategy
        ))
        .routes(routes!(strategies::submit_strategy_chat))
        .routes(routes!(strategies::get_strategy_task))
        .routes(routes!(strategies::list_strategy_task_notes))
        .routes(routes!(strategies::list_strategy_tasks))
        .routes(routes!(tasks::list_tasks))
        .routes(routes!(
            strategies::get_investable_amount,
            strategies::put_investable_amount
        ))
        .routes(routes!(
            agent_config::list_agent_configs,
            agent_config::create_agent_config
        ))
        .routes(routes!(
            agent_config::get_agent_config,
            agent_config::delete_agent_config
        ))
        .routes(routes!(
            agent_config::get_agents_md,
            agent_config::put_agents_md
        ))
        .routes(routes!(agent_config::get_skills, agent_config::put_skills))
        .routes(routes!(agent_config::put_skill, agent_config::delete_skill))
        .routes(routes!(
            agent_config::get_agent_graph,
            agent_config::put_agent_graph
        ))
        .routes(routes!(agent_config::get_agent_config_bundle))
        .routes(routes!(refs::list_stocks))
        .routes(routes!(refs::get_stock))
        .routes(routes!(refs::list_indicators))
        .routes(routes!(refs::get_indicator))
        .routes(routes!(refs::resolve_refs))
        .routes(routes!(notes::list_notes, notes::create_note))
        .routes(routes!(
            notes::get_note,
            notes::update_note,
            notes::delete_note
        ))
        .routes(routes!(note_versions::list_note_versions))
        .routes(routes!(note_versions::get_note_version))
        .routes(routes!(note_versions::approve_note_version))
        .routes(routes!(note_versions::reject_note_version))
        .routes(routes!(note_versions::make_note_version_current))
        .routes(routes!(note_versions::list_pending_note_versions))
        .routes(routes!(note_links::get_note_links))
        .routes(routes!(note_predictions::list_note_predictions))
        .routes(routes!(
            annotations::list_annotations,
            annotations::create_annotation
        ))
        .routes(routes!(
            annotations::get_annotation,
            annotations::update_annotation,
            annotations::delete_annotation
        ))
        .routes(routes!(annotations::approve_annotation))
        .routes(routes!(annotations::reject_annotation))
        .routes(routes!(comments::list_comments, comments::create_comment))
        .routes(routes!(comments::update_comment, comments::delete_comment))
        .routes(routes!(history::list_history))
        .routes(routes!(history::get_history))
        .routes(routes!(trades::trades_summary))
        .routes(routes!(trades::list_trades, trades::create_trade))
        .routes(routes!(
            trades::get_trade,
            trades::update_trade,
            trades::delete_trade
        ))
        .routes(routes!(
            trade_notes::list_trade_notes,
            trade_notes::create_trade_note
        ))
        .routes(routes!(trade_notes::delete_trade_note))
        .routes(routes!(
            triggers::list_strategy_triggers,
            triggers::create_strategy_trigger
        ))
        .routes(routes!(
            triggers::get_trigger,
            triggers::update_trigger,
            triggers::delete_trigger
        ))
        .routes(routes!(imports::sbi_preview))
        .routes(routes!(imports::sbi_commit))
        .routes(routes!(
            custom_indicators::list_global_indicators,
            custom_indicators::create_global_indicator
        ))
        .routes(routes!(
            custom_indicators::get_indicator,
            custom_indicators::update_indicator,
            custom_indicators::delete_indicator
        ))
        .routes(routes!(
            custom_indicators::list_strategy_indicators,
            custom_indicators::create_strategy_indicator
        ))
        .routes(routes!(custom_indicators::get_strategy_indicator))
        .routes(routes!(custom_indicators::preview_indicator))
        .routes(routes!(
            rss_feeds::list_rss_feeds,
            rss_feeds::create_rss_feed
        ))
        .routes(routes!(
            rss_feeds::get_rss_feed,
            rss_feeds::update_rss_feed,
            rss_feeds::delete_rss_feed
        ))
        .routes(routes!(
            note_kinds::list_note_kinds,
            note_kinds::create_note_kind
        ))
        .routes(routes!(
            note_kinds::update_note_kind,
            note_kinds::delete_note_kind
        ))
        .routes(routes!(
            group_axes::list_group_axes,
            group_axes::create_group_axis
        ))
        .routes(routes!(
            group_axes::get_group_axis,
            group_axes::update_group_axis,
            group_axes::delete_group_axis
        ))
        .routes(routes!(agent_options::get_agent_models))
        .routes(routes!(agent_options::get_agent_tools))
        .routes(routes!(config::get_config))
        .routes(routes!(
            risk_policy::get_account_risk_policy,
            risk_policy::put_account_risk_policy
        ))
}
