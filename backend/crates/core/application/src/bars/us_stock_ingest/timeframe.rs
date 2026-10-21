use chrono::{DateTime, TimeZone, Utc};
use core_domain::bar::Timeframe;

use super::super::types::{UsStockBarTarget, UsStockSipBarRefetchTimeframe};
use super::{
    UsStockBarTimeframe, us_stock_hourly_history_cutoff, us_stock_sip_bars_history_cutoff,
};

impl UsStockBarTimeframe {
    pub(super) fn timeframe(self) -> Timeframe {
        match self {
            Self::Daily => Timeframe::Daily,
            Self::Minute => Timeframe::Minute,
            Self::Hourly => Timeframe::Hourly,
        }
    }

    pub(super) const fn as_sip_refetch_timeframe(self) -> UsStockSipBarRefetchTimeframe {
        match self {
            Self::Daily => UsStockSipBarRefetchTimeframe::Daily,
            Self::Hourly => UsStockSipBarRefetchTimeframe::Hourly,
            Self::Minute => UsStockSipBarRefetchTimeframe::Minute,
        }
    }

    pub(super) fn history_cursor(self, target: &UsStockBarTarget) -> Option<DateTime<Utc>> {
        match self {
            Self::Daily => target.latest_daily_bar,
            Self::Minute => target.latest_minute_bar,
            Self::Hourly => target.latest_hourly_bar,
        }
    }

    pub(super) fn earliest_bar(self, target: &UsStockBarTarget) -> Option<DateTime<Utc>> {
        match self {
            Self::Daily => target.earliest_daily_bar,
            Self::Minute => target.earliest_minute_bar,
            Self::Hourly => target.earliest_hourly_bar,
        }
    }

    pub(super) fn history_start(self, now: DateTime<Utc>) -> DateTime<Utc> {
        match self {
            Self::Daily => Utc.timestamp_opt(0, 0).single().unwrap_or(now),
            Self::Minute => us_stock_hourly_history_cutoff(now),
            Self::Hourly => Utc
                .with_ymd_and_hms(2016, 1, 1, 0, 0, 0)
                .single()
                .unwrap_or(now),
        }
    }

    pub(super) fn history_end(self, now: DateTime<Utc>) -> DateTime<Utc> {
        let end = match self {
            Self::Hourly => us_stock_hourly_history_cutoff(now),
            Self::Daily | Self::Minute => now,
        };
        end.min(us_stock_sip_bars_history_cutoff(now))
    }

    pub(super) fn is_complete(self, target: &UsStockBarTarget) -> bool {
        // API は初回の履歴取得に使い、その後の時間足は古くなった分足から補う。
        self == Self::Hourly && target.has_hourly_history_backfill()
    }
}
