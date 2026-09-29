use std::sync::Arc;

use core_application::change_history::SharedChangeHistoryPort;
use core_application::custom_indicator::{
    CustomIndicatorUseCases, SharedCustomIndicatorRepository,
};
use core_application::news::{NewsUseCases, SharedNewsItemRepository};
use core_application::note::{NoteUseCases, SharedNoteRepository};
use core_application::rss_feed::{
    RssFeedUseCases, SharedRssFeedRepository, SharedRssFeedUrlValidator,
};
use core_application::strategy::{
    SharedStrategyRepository, SharedStrategySummaryQuery, StrategyUseCases,
};
use core_application::strategy_existence::SharedStrategyExistence;
use core_application::strategy_task::{SharedStrategyTaskRepository, StrategyTaskUseCases};
use core_application::trade::{SharedTradeRepository, TradeUseCases};
use core_application::unit_of_work::SharedUnitOfWork;
use gateway_postgres::{
    DatabaseHandle, PostgresChangeHistory, PostgresCustomIndicatorRepository,
    PostgresNewsItemRepository, PostgresNoteRepository, PostgresRssFeedRepository,
    PostgresStrategyExistence, PostgresStrategyRepository, PostgresStrategySummaryQuery,
    PostgresStrategyTaskRepository, PostgresTradeRepository, PostgresUnitOfWork,
};
use gateway_rss::HttpRssFeedUrlValidator;

#[derive(Clone)]
pub struct UseCases {
    pub notes: NoteUseCases,
    pub strategies: StrategyUseCases,
    pub trades: TradeUseCases,
    pub strategy_tasks: StrategyTaskUseCases,
    pub custom_indicators: CustomIndicatorUseCases,
    pub rss_feeds: RssFeedUseCases,
    pub news: NewsUseCases,
}

pub fn build_use_cases(db: impl Into<DatabaseHandle>) -> UseCases {
    let db = db.into();
    let unit_of_work: SharedUnitOfWork = Arc::new(PostgresUnitOfWork::new(db.clone()));
    let repository: SharedTradeRepository = Arc::new(PostgresTradeRepository::new(db.clone()));
    let custom_indicator_repository: SharedCustomIndicatorRepository =
        Arc::new(PostgresCustomIndicatorRepository::new(db.clone()));
    let rss_feed_repository: SharedRssFeedRepository =
        Arc::new(PostgresRssFeedRepository::new(db.clone()));
    let news_item_repository: SharedNewsItemRepository =
        Arc::new(PostgresNewsItemRepository::new(db.clone()));
    let strategy_existence: SharedStrategyExistence = Arc::new(PostgresStrategyExistence);
    let change_history: SharedChangeHistoryPort = Arc::new(PostgresChangeHistory);
    let note_repository: SharedNoteRepository = Arc::new(PostgresNoteRepository::new());
    let notes = NoteUseCases::new(
        unit_of_work.clone(),
        note_repository,
        strategy_existence.clone(),
        change_history.clone(),
    );
    let strategy_repository: SharedStrategyRepository =
        Arc::new(PostgresStrategyRepository::new(db.clone()));
    let strategy_summary_query: SharedStrategySummaryQuery =
        Arc::new(PostgresStrategySummaryQuery::new(db.clone()));
    let strategies = StrategyUseCases::new(
        unit_of_work.clone(),
        strategy_repository,
        strategy_summary_query,
        change_history.clone(),
    );

    let trades = TradeUseCases::new(
        unit_of_work.clone(),
        repository,
        strategy_existence.clone(),
        change_history.clone(),
    );
    let custom_indicators = CustomIndicatorUseCases::new(
        unit_of_work.clone(),
        custom_indicator_repository,
        strategy_existence,
        change_history,
    );
    let rss_feed_url_validator: SharedRssFeedUrlValidator = Arc::new(HttpRssFeedUrlValidator);
    let rss_feeds = RssFeedUseCases::new(
        unit_of_work.clone(),
        rss_feed_repository.clone(),
        rss_feed_url_validator,
    );
    let news = NewsUseCases::new(unit_of_work, rss_feed_repository, news_item_repository);

    let strategy_tasks = build_strategy_task_use_cases(db);

    UseCases {
        notes,
        strategies,
        trades,
        strategy_tasks,
        custom_indicators,
        rss_feeds,
        news,
    }
}

pub fn build_strategy_task_use_cases(db: impl Into<DatabaseHandle>) -> StrategyTaskUseCases {
    let db = db.into();
    let task_unit_of_work: SharedUnitOfWork = Arc::new(PostgresUnitOfWork::new(db.clone()));
    let task_repository: SharedStrategyTaskRepository =
        Arc::new(PostgresStrategyTaskRepository::new(db));
    StrategyTaskUseCases::new(task_unit_of_work, task_repository)
}
