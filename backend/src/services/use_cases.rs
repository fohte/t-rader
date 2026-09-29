use std::sync::Arc;

use core_application::annotation::{AnnotationUseCases, SharedAnnotationRepository};
use core_application::change_history::SharedChangeHistoryPort;
use core_application::comment::{CommentUseCases, SharedCommentRepository};
use core_application::custom_indicator::{
    CustomIndicatorUseCases, SharedCustomIndicatorRepository,
};
use core_application::strategy::{
    SharedStrategyRepository, SharedStrategySummaryQuery, StrategyUseCases,
};
use core_application::strategy_existence::SharedStrategyExistence;
use core_application::trade::{SharedTradeRepository, TradeUseCases};
use core_application::unit_of_work::SharedUnitOfWork;
use gateway_postgres::{
    DatabaseHandle, PostgresAnnotationRepository, PostgresChangeHistory, PostgresCommentRepository,
    PostgresCustomIndicatorRepository, PostgresStrategyExistence, PostgresStrategyRepository,
    PostgresStrategySummaryQuery, PostgresTradeRepository, PostgresUnitOfWork,
};

#[derive(Clone)]
pub struct UseCases {
    pub annotations: AnnotationUseCases,
    pub comments: CommentUseCases,
    pub strategies: StrategyUseCases,
    pub trades: TradeUseCases,
    pub custom_indicators: CustomIndicatorUseCases,
}

pub fn build_use_cases(db: impl Into<DatabaseHandle>) -> UseCases {
    let db = db.into();
    let unit_of_work: SharedUnitOfWork = Arc::new(PostgresUnitOfWork::new(db.clone()));
    let annotation_repository: SharedAnnotationRepository = Arc::new(PostgresAnnotationRepository);
    let comment_repository: SharedCommentRepository =
        Arc::new(PostgresCommentRepository::new(db.clone()));
    let repository: SharedTradeRepository = Arc::new(PostgresTradeRepository::new(db.clone()));
    let custom_indicator_repository: SharedCustomIndicatorRepository =
        Arc::new(PostgresCustomIndicatorRepository::new(db.clone()));
    let strategy_existence: SharedStrategyExistence = Arc::new(PostgresStrategyExistence);
    let change_history: SharedChangeHistoryPort = Arc::new(PostgresChangeHistory);
    let annotations = AnnotationUseCases::new(
        unit_of_work.clone(),
        annotation_repository,
        strategy_existence.clone(),
        change_history.clone(),
    );
    let comments = CommentUseCases::new(
        unit_of_work.clone(),
        comment_repository,
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
        unit_of_work,
        custom_indicator_repository,
        strategy_existence,
        change_history,
    );

    UseCases {
        annotations,
        comments,
        strategies,
        trades,
        custom_indicators,
    }
}
