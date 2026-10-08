use chrono::{DateTime, FixedOffset, NaiveDate};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperAccount {
    pub id: Uuid,
    pub name: String,
    pub strategy_id: Uuid,
    pub purpose: String,
    pub initial_cash_jpy: Decimal,
    pub benchmark_stock_id: Option<String>,
    pub started_on: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPaperAccount {
    pub name: String,
    pub strategy_id: Uuid,
    pub purpose: String,
    pub initial_cash_jpy: Decimal,
    pub benchmark_stock_id: Option<String>,
    pub started_on: NaiveDate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperOrderSide {
    Buy,
    Sell,
}

impl PaperOrderSide {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "buy" => Some(Self::Buy),
            "sell" => Some(Self::Sell),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperOrder {
    pub id: Uuid,
    pub account_id: Uuid,
    pub stock_id: String,
    pub side: PaperOrderSide,
    pub qty: i64,
    pub note_version_id: Uuid,
    pub ordered_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPaperOrder {
    pub account_id: Uuid,
    pub stock_id: String,
    pub side: PaperOrderSide,
    pub qty: i64,
    pub note_version_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperOrderOutcome {
    Filled,
    Rejected,
}

impl PaperOrderOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Filled => "filled",
            Self::Rejected => "rejected",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "filled" => Some(Self::Filled),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperOrderRejectReason {
    DailyBarUnavailable,
    InsufficientCash,
    InsufficientShares,
}

impl PaperOrderRejectReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DailyBarUnavailable => "daily_bar_unavailable",
            Self::InsufficientCash => "insufficient_cash",
            Self::InsufficientShares => "insufficient_shares",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "daily_bar_unavailable" => Some(Self::DailyBarUnavailable),
            "insufficient_cash" => Some(Self::InsufficientCash),
            "insufficient_shares" => Some(Self::InsufficientShares),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaperOrderResult {
    Filled {
        order_id: Uuid,
        fill_date: NaiveDate,
        fill_price: Decimal,
        decided_at: DateTime<FixedOffset>,
    },
    Rejected {
        order_id: Uuid,
        reject_reason: PaperOrderRejectReason,
        decided_at: DateTime<FixedOffset>,
    },
}

impl PaperOrderResult {
    pub fn order_id(&self) -> Uuid {
        match self {
            Self::Filled { order_id, .. } | Self::Rejected { order_id, .. } => *order_id,
        }
    }

    pub fn outcome(&self) -> PaperOrderOutcome {
        match self {
            Self::Filled { .. } => PaperOrderOutcome::Filled,
            Self::Rejected { .. } => PaperOrderOutcome::Rejected,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperOrderWithResult {
    pub order: PaperOrder,
    pub result: Option<PaperOrderResult>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct PaperTradeFillStats {
    pub filled: usize,
    pub rejected: usize,
}
