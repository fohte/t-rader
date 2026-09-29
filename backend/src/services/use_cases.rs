use std::sync::Arc;

use core_application::account_risk_policy::{
    AccountRiskPolicyUseCases, SharedAccountRiskPolicyRepository,
};
use core_application::change_history::SharedChangeHistoryPort;
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
    DatabaseHandle, PostgresAccountRiskPolicyRepository, PostgresChangeHistory,
    PostgresCustomIndicatorRepository, PostgresStrategyExistence, PostgresStrategyRepository,
    PostgresStrategySummaryQuery, PostgresTradeRepository, PostgresUnitOfWork,
};

#[derive(Clone)]
pub struct UseCases {
    pub account_risk_policies: AccountRiskPolicyUseCases,
    pub strategies: StrategyUseCases,
    pub trades: TradeUseCases,
    pub custom_indicators: CustomIndicatorUseCases,
}

pub fn build_use_cases(db: impl Into<DatabaseHandle>) -> UseCases {
    let db = db.into();
    let unit_of_work: SharedUnitOfWork = Arc::new(PostgresUnitOfWork::new(db.clone()));
    let account_risk_policy_repository: SharedAccountRiskPolicyRepository =
        Arc::new(PostgresAccountRiskPolicyRepository::new(db.clone()));
    let repository: SharedTradeRepository = Arc::new(PostgresTradeRepository::new(db.clone()));
    let custom_indicator_repository: SharedCustomIndicatorRepository =
        Arc::new(PostgresCustomIndicatorRepository::new(db.clone()));
    let strategy_existence: SharedStrategyExistence = Arc::new(PostgresStrategyExistence);
    let change_history: SharedChangeHistoryPort = Arc::new(PostgresChangeHistory);
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
        account_risk_policies: AccountRiskPolicyUseCases::new(account_risk_policy_repository),
        strategies,
        trades,
        custom_indicators,
    }
}
