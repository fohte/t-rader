use std::sync::Arc;

use core_application::annotation::{AnnotationUseCases, SharedAnnotationRepository};
use core_application::change_history::SharedChangeHistoryPort;
use core_application::comment::{CommentUseCases, SharedCommentRepository};
use core_application::strategy_existence::SharedStrategyExistence;
use core_application::trade::{SharedTradeRepository, TradeUseCases};
use core_application::unit_of_work::SharedUnitOfWork;
use gateway_postgres::{
    DatabaseHandle, PostgresAnnotationRepository, PostgresChangeHistory, PostgresCommentRepository,
    PostgresStrategyExistence, PostgresTradeRepository, PostgresUnitOfWork,
};

#[derive(Clone)]
pub struct UseCases {
    pub annotations: AnnotationUseCases,
    pub comments: CommentUseCases,
    pub trades: TradeUseCases,
}

pub fn build_use_cases(db: impl Into<DatabaseHandle>) -> UseCases {
    let db = db.into();
    let unit_of_work: SharedUnitOfWork = Arc::new(PostgresUnitOfWork::new(db.clone()));
    let annotation_repository: SharedAnnotationRepository = Arc::new(PostgresAnnotationRepository);
    let comment_repository: SharedCommentRepository =
        Arc::new(PostgresCommentRepository::new(db.clone()));
    let repository: SharedTradeRepository = Arc::new(PostgresTradeRepository::new(db.clone()));
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
    let trades = TradeUseCases::new(unit_of_work, repository, strategy_existence, change_history);

    UseCases {
        annotations,
        comments,
        trades,
    }
}
