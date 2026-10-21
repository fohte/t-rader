use chrono::{Days, Months, NaiveDate};

use super::types::WidgetPeriod;

const MAX_RANGE_MONTHS: u32 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetTimeRange {
    ThreeMonths,
    OneYear,
    Dates { from: NaiveDate, to: NaiveDate },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedWidgetTimeRange {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub query_from: NaiveDate,
}

impl ResolvedWidgetTimeRange {
    pub fn input_calendar_days(self) -> u64 {
        (self.to - self.query_from).num_days() as u64 + 1
    }
}

impl WidgetTimeRange {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "3m" => return Ok(Self::ThreeMonths),
            "1y" => return Ok(Self::OneYear),
            _ => {}
        }

        let Some((from, to)) = value.split_once("..") else {
            return Err("range must be `3m`, `1y`, or `YYYY-MM-DD..YYYY-MM-DD`".to_string());
        };
        let from = NaiveDate::parse_from_str(from, "%Y-%m-%d")
            .map_err(|_| "range start must use `YYYY-MM-DD`".to_string())?;
        let to = NaiveDate::parse_from_str(to, "%Y-%m-%d")
            .map_err(|_| "range end must use `YYYY-MM-DD`".to_string())?;
        if from > to {
            return Err("range start must not be after range end".to_string());
        }
        if from < earliest_date_in_range(to) {
            return Err("range must not exceed one year".to_string());
        }
        Ok(Self::Dates { from, to })
    }

    pub fn is_explicit(self) -> bool {
        matches!(self, Self::Dates { .. })
    }

    pub fn resolve(
        self,
        to: NaiveDate,
        period: Option<WidgetPeriod>,
    ) -> Result<ResolvedWidgetTimeRange, String> {
        let from = match self {
            Self::ThreeMonths => to
                .checked_sub_months(Months::new(3))
                .ok_or_else(|| "range start is outside the supported date range".to_string())?,
            Self::OneYear => to
                .checked_sub_months(Months::new(MAX_RANGE_MONTHS))
                .ok_or_else(|| "range start is outside the supported date range".to_string())?,
            Self::Dates { from, to: range_to } => {
                if range_to != to {
                    return Err("resolved date must match the explicit range end".to_string());
                }
                from
            }
        };
        let warmup_days = period
            .map(|period| (period.trading_days() + period.trading_days() / 4 + 21) as u64)
            .unwrap_or_default();
        let query_from = from
            .checked_sub_days(Days::new(warmup_days))
            .ok_or_else(|| "range warmup is outside the supported date range".to_string())?;
        Ok(ResolvedWidgetTimeRange {
            from,
            to,
            query_from,
        })
    }
}

fn earliest_date_in_range(to: NaiveDate) -> NaiveDate {
    to.checked_sub_months(Months::new(MAX_RANGE_MONTHS))
        .unwrap_or(NaiveDate::MIN)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use rstest::rstest;

    use crate::widget::WidgetPeriod;

    use super::{ResolvedWidgetTimeRange, WidgetTimeRange};

    #[rstest]
    #[case::three_months("3m", Ok(WidgetTimeRange::ThreeMonths))]
    #[case::one_year("1y", Ok(WidgetTimeRange::OneYear))]
    #[case::explicit_dates(
        "2097-01-05..2098-01-05",
        Ok(WidgetTimeRange::Dates {
            from: NaiveDate::from_ymd_opt(2097, 1, 5).expect("valid date"),
            to: NaiveDate::from_ymd_opt(2098, 1, 5).expect("valid date"),
        })
    )]
    #[case::invalid_format(
        "tomorrow",
        Err("range must be `3m`, `1y`, or `YYYY-MM-DD..YYYY-MM-DD`".to_string())
    )]
    #[case::reversed_dates(
        "2098-01-06..2098-01-05",
        Err("range start must not be after range end".to_string())
    )]
    #[case::too_long(
        "2096-12-31..2098-01-05",
        Err("range must not exceed one year".to_string())
    )]
    fn parses_supported_widget_time_ranges(
        #[case] input: &str,
        #[case] expected: Result<WidgetTimeRange, String>,
    ) {
        assert_eq!(WidgetTimeRange::parse(input), expected);
    }

    #[rstest]
    #[case::sixty_day_warmup(
        WidgetPeriod::SixtyDays,
        NaiveDate::from_ymd_opt(2097, 10, 1).expect("valid date")
    )]
    #[case::one_day_warmup(
        WidgetPeriod::OneDay,
        NaiveDate::from_ymd_opt(2097, 12, 14).expect("valid date")
    )]
    fn resolves_range_with_calendar_warmup(
        #[case] period: WidgetPeriod,
        #[case] expected_query_from: NaiveDate,
    ) {
        let range = WidgetTimeRange::Dates {
            from: NaiveDate::from_ymd_opt(2098, 1, 5).expect("valid date"),
            to: NaiveDate::from_ymd_opt(2098, 1, 5).expect("valid date"),
        };

        assert_eq!(
            range.resolve(
                NaiveDate::from_ymd_opt(2098, 1, 5).expect("valid date"),
                Some(period)
            ),
            Ok(ResolvedWidgetTimeRange {
                from: NaiveDate::from_ymd_opt(2098, 1, 5).expect("valid date"),
                to: NaiveDate::from_ymd_opt(2098, 1, 5).expect("valid date"),
                query_from: expected_query_from,
            })
        );
    }
}
