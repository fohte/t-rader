CREATE TEMP TABLE synthetic_stocks AS
SELECT
    n,
    'X'
        || substr('0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ', ((n - 1) / 1296 % 36)::integer + 1, 1)
        || substr('0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ', ((n - 1) / 36 % 36)::integer + 1, 1)
        || substr('0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ', ((n - 1) % 36)::integer + 1, 1) AS id,
    'Synthetic Stock ' || lpad(n::text, 5, '0') AS name
FROM generate_series(1, 15000) AS n;

CREATE TEMP TABLE synthetic_trading_days AS
SELECT
    day::date AS bar_date,
    row_number() OVER (ORDER BY day)::integer AS day_number
FROM generate_series(
    (current_date - INTERVAL '3 years')::date::timestamp,
    current_date::timestamp,
    INTERVAL '1 day'
) AS days(day)
WHERE extract(isodow FROM day) BETWEEN 1 AND 5;

-- bars は instruments を、グループ所属は stock を参照するため、両方に同じ ID を登録する。
INSERT INTO stock (id, name, market, product_category)
SELECT id, name, 'TSE', NULL
FROM synthetic_stocks;

INSERT INTO instruments (id, name, market, sector)
SELECT id, name, 'TSE', NULL
FROM synthetic_stocks;

INSERT INTO group_axis (key, name, description)
VALUES
    ('sample-axis', 'Sample Axis', 'Synthetic sample classification axis'),
    ('demo-axis', 'Demo Axis', 'Synthetic demo classification axis'),
    ('other-axis', 'Sample Axis', 'Synthetic alternate classification axis');

CREATE TEMP TABLE synthetic_groups AS
WITH axes(axis_key, group_prefix, filler_group_name) AS (
    VALUES
        ('sample-axis', 'sample', 'Sample Group'),
        ('demo-axis', 'demo', 'Demo Group'),
        ('other-axis', 'other', 'Sample Group')
),
fixed_groups(axis_key, group_number, group_key, group_name) AS (
    VALUES
        ('sample-axis', 1, 'sample-group', 'Sample Group'),
        ('demo-axis', 1, 'group-alpha', 'Group Alpha'),
        ('demo-axis', 2, 'demo-group', 'Sample Group'),
        ('other-axis', 1, 'other-group', 'Sample Group')
)
SELECT
    axes.axis_key,
    groups.group_number,
    coalesce(
        fixed_groups.group_key,
        axes.group_prefix || '-' || lpad(groups.group_number::text, 2, '0')
    ) AS group_key,
    coalesce(
        fixed_groups.group_name,
        axes.filler_group_name || ' ' || lpad(groups.group_number::text, 2, '0')
    ) AS group_name
FROM axes
CROSS JOIN generate_series(1, 30) AS groups(group_number)
LEFT JOIN fixed_groups
  ON fixed_groups.axis_key = axes.axis_key
 AND fixed_groups.group_number = groups.group_number;

INSERT INTO stock_group (axis_id, key, name)
SELECT group_axis.id, synthetic_groups.group_key, synthetic_groups.group_name
FROM synthetic_groups
JOIN group_axis ON group_axis.key = synthetic_groups.axis_key;

INSERT INTO stock_group_member (stock_id, group_id)
SELECT synthetic_stocks.id, stock_group.id
FROM synthetic_stocks
JOIN synthetic_groups
  ON (synthetic_stocks.n - 1) % 30 = synthetic_groups.group_number - 1
JOIN group_axis ON group_axis.key = synthetic_groups.axis_key
JOIN stock_group
  ON stock_group.axis_id = group_axis.id
 AND stock_group.key = synthetic_groups.group_key;

WITH hourly_trading_days AS (
    SELECT day::date AS bar_date
    FROM generate_series(
        (current_date - INTERVAL '90 days')::date::timestamp,
        current_date::timestamp,
        INTERVAL '1 day'
    ) AS days(day)
    WHERE extract(isodow FROM day) BETWEEN 1 AND 5
),
recent_minute_trading_days AS (
    SELECT day::date AS bar_date
    FROM generate_series(
        (current_date - INTERVAL '14 days')::date::timestamp,
        current_date::timestamp,
        INTERVAL '1 day'
    ) AS days(day)
    WHERE extract(isodow FROM day) BETWEEN 1 AND 5
    ORDER BY day DESC
    LIMIT 10
),
bar_source AS (
    SELECT
        synthetic_stocks.id AS instrument_id,
        '1d' AS timeframe,
        synthetic_trading_days.bar_date::timestamp AT TIME ZONE 'UTC' AS timestamp,
        synthetic_stocks.n,
        synthetic_trading_days.day_number AS price_step
    FROM synthetic_stocks
    CROSS JOIN synthetic_trading_days
    UNION ALL
    SELECT
        synthetic_stocks.id,
        '1h',
        (hourly_trading_days.bar_date + TIME '09:00' + make_interval(hours => hourly_bars.hour)) AT TIME ZONE 'UTC',
        synthetic_stocks.n,
        hourly_bars.hour
    FROM synthetic_stocks
    JOIN hourly_trading_days ON synthetic_stocks.n <= 300
    CROSS JOIN LATERAL generate_series(0, 6) AS hourly_bars(hour)
    UNION ALL
    SELECT
        synthetic_stocks.id,
        '1m',
        (recent_minute_trading_days.bar_date + TIME '09:00' + make_interval(mins => minute_bars.minute)) AT TIME ZONE 'UTC',
        synthetic_stocks.n,
        minute_bars.minute
    FROM synthetic_stocks
    JOIN recent_minute_trading_days ON synthetic_stocks.n <= 25
    CROSS JOIN LATERAL generate_series(0, 389) AS minute_bars(minute)
),
bar_values AS (
    SELECT
        instrument_id,
        timeframe,
        timestamp,
        n,
        price_step,
        (50 + n % 300 + price_step % 20 / 10.0)::numeric AS close
    FROM bar_source
)
INSERT INTO bars (
    instrument_id,
    timeframe,
    timestamp,
    open,
    high,
    low,
    close,
    volume,
    adjustment_factor
)
SELECT
    instrument_id,
    timeframe,
    timestamp,
    close - 0.1,
    close + 0.2,
    close - 0.3,
    close,
    1000 + n * 17 + price_step,
    1
FROM bar_values;

INSERT INTO valuation (code, date, mkt_cap)
SELECT synthetic_stocks.id || '0', latest_trading_day.latest_trading_day, 1000 + synthetic_stocks.n * 10
FROM synthetic_stocks
CROSS JOIN (SELECT max(bar_date) AS latest_trading_day FROM synthetic_trading_days) AS latest_trading_day;

SELECT compress_chunk(format('%I.%I', chunk_schema, chunk_name)::regclass)
FROM timescaledb_information.chunks
WHERE hypertable_schema = 'public'
  AND hypertable_name = 'bars'
  AND range_end < now() - INTERVAL '7 days'
  AND NOT is_compressed;

ANALYZE stock;
ANALYZE instruments;
ANALYZE bars;
ANALYZE valuation;
ANALYZE group_axis;
ANALYZE stock_group;
ANALYZE stock_group_member;
