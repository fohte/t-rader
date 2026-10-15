pub(super) const GROUP_MEMBERSHIP_CTES: &str = r#"
    axis_groups AS (
        SELECT axis.key AS axis_key,
               group_row.id AS group_id,
               group_row.key AS group_key,
               group_row.name AS group_name
        FROM requested_axes
        JOIN group_axis AS axis ON axis.key = requested_axes.axis_key
        JOIN stock_group AS group_row ON group_row.axis_id = axis.id
    ),
    group_members AS (
        SELECT axis_groups.axis_key,
               axis_groups.group_id,
               axis_groups.group_key,
               axis_groups.group_name,
               member.stock_id
        FROM axis_groups
        LEFT JOIN stock_group_member AS member
          ON member.group_id = axis_groups.group_id
    )
"#;

pub(super) const STOCK_CHANGE_CTES: &str = r#"
    period_bars AS (
        SELECT bars.instrument_id, bars.timestamp, bars.close, bars.volume
        FROM stock_change_windows
        JOIN bars ON bars.instrument_id = stock_change_windows.stock_id
        WHERE bars.timeframe = '1d'
          AND bars.timestamp >= stock_change_windows.start_at
          AND bars.timestamp < stock_change_windows.end_before
    ),
    stock_changes AS (
        SELECT stock_change_windows.stock_id,
               end_prices.close / NULLIF(start_prices.close, 0) - 1 AS change_rate
        FROM stock_change_windows
        CROSS JOIN LATERAL (
            SELECT period_bars.close
            FROM period_bars
            WHERE period_bars.instrument_id = stock_change_windows.stock_id
            ORDER BY period_bars.timestamp DESC
            LIMIT 1
        ) AS end_prices
        CROSS JOIN LATERAL (
            SELECT bars.close
            FROM bars
            WHERE bars.instrument_id = stock_change_windows.stock_id
              AND bars.timeframe = '1d'
              AND bars.timestamp < stock_change_windows.start_at
            ORDER BY bars.timestamp DESC
            LIMIT 1
        ) AS start_prices
        WHERE start_prices.close <> 0
    )
"#;

/// `stock_change_window_rows` は `stock_id`, `start_at`, `end_before` を返す。
pub(super) fn with_shared_market_ctes(
    requested_axes: &str,
    before_stock_change_windows: &str,
    stock_change_window_rows: &str,
    after_stock_changes: &str,
) -> String {
    format!(
        "WITH requested_axes AS ({requested_axes}), {GROUP_MEMBERSHIP_CTES}, {before_stock_change_windows}, stock_change_windows AS ({stock_change_window_rows}), {STOCK_CHANGE_CTES}, {after_stock_changes}"
    )
}
