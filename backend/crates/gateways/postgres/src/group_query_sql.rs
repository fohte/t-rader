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
    group_members AS NOT MATERIALIZED (
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

/// CTE を依存順に連結し、後続の断片から `stock_changes(stock_id, change_rate)` を参照できるようにする。
pub(super) fn with_shared_market_ctes(
    requested_axes: &str,
    before_stock_change_ctes: &str,
    stock_change_ctes: &str,
    after_stock_change_ctes: &str,
) -> String {
    format!(
        "WITH requested_axes AS ({requested_axes}), {GROUP_MEMBERSHIP_CTES}, {before_stock_change_ctes}, {stock_change_ctes}, {after_stock_change_ctes}"
    )
}
