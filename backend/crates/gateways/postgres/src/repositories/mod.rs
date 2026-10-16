pub mod bars;
pub mod margin_alert;
pub mod margin_interest;
pub mod short_ratio;
pub mod short_sale_report;

pub(crate) fn max_rows_per_statement(columns_per_row: usize) -> usize {
    u16::MAX as usize / columns_per_row
}
