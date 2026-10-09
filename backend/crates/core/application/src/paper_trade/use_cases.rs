use core_domain::stock_id::has_country_prefix;

use crate::unit_of_work::SharedUnitOfWork;

use super::{
    PaperTradeUseCaseError,
    repository::SharedPaperTradeRepository,
    types::{NewPaperAccount, NewPaperOrder, PaperAccount, PaperOrder},
};

#[derive(Clone)]
pub struct PaperTradeUseCases {
    pub(super) unit_of_work: SharedUnitOfWork,
    pub(super) repository: SharedPaperTradeRepository,
    pub(super) bars: crate::bars::SharedBarsRepository,
}

impl PaperTradeUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedPaperTradeRepository,
        bars: crate::bars::SharedBarsRepository,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            bars,
        }
    }

    pub async fn create_account(
        &self,
        mut account: NewPaperAccount,
    ) -> Result<PaperAccount, PaperTradeUseCaseError> {
        account.name = account.name.trim().to_string();
        account.purpose = account.purpose.trim().to_string();
        if account.name.is_empty() {
            return Err(PaperTradeUseCaseError::Validation(
                "name must not be empty".into(),
            ));
        }
        if account.purpose.is_empty() {
            return Err(PaperTradeUseCaseError::Validation(
                "purpose must not be empty".into(),
            ));
        }
        if account.initial_cash_jpy <= rust_decimal::Decimal::ZERO {
            return Err(PaperTradeUseCaseError::Validation(
                "initial_cash_jpy must be greater than zero".into(),
            ));
        }
        if account
            .benchmark_stock_id
            .as_deref()
            .is_some_and(has_country_prefix)
        {
            return Err(PaperTradeUseCaseError::Validation(
                "benchmark_stock_id must identify a Japanese stock".into(),
            ));
        }

        let transaction = self.unit_of_work.begin().await?;
        let created = self
            .repository
            .insert_account(&transaction, account)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(created)
    }

    pub async fn record_order(
        &self,
        order: NewPaperOrder,
    ) -> Result<PaperOrder, PaperTradeUseCaseError> {
        if has_country_prefix(&order.stock_id) {
            return Err(PaperTradeUseCaseError::Validation(
                "paper orders only support Japanese stocks".into(),
            ));
        }
        if order.qty <= 0 || order.qty % 100 != 0 {
            return Err(PaperTradeUseCaseError::Validation(
                "qty must be a positive multiple of 100".into(),
            ));
        }

        let transaction = self.unit_of_work.begin().await?;
        if !self
            .repository
            .account_exists(&transaction, order.account_id)
            .await?
        {
            return Err(PaperTradeUseCaseError::AccountNotFound(order.account_id));
        }
        let created = self.repository.insert_order(&transaction, order).await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(created)
    }
}
