CREATE TEMP TABLE synthetic_stocks AS
SELECT
    n,
    'X'
        || substr('0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ', ((n - 1) / 1296 % 36)::integer + 1, 1)
        || substr('0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ', ((n - 1) / 36 % 36)::integer + 1, 1)
        || substr('0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ', ((n - 1) % 36)::integer + 1, 1) AS id,
    'Synthetic Stock ' || lpad(n::text, 5, '0') AS name
FROM generate_series(1, 15000) AS n;

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

INSERT INTO stock_group (axis_id, key, name)
SELECT group_axis.id, group_data.key, group_data.name
FROM (
    VALUES
        ('sample-axis', 'sample-group', 'Sample Group'),
        ('demo-axis', 'group-alpha', 'Group Alpha'),
        ('demo-axis', 'demo-group', 'Sample Group'),
        ('other-axis', 'other-group', 'Sample Group')
) AS group_data(axis_key, key, name)
JOIN group_axis ON group_axis.key = group_data.axis_key;

INSERT INTO stock_group_member (stock_id, group_id)
SELECT synthetic_stocks.id, stock_group.id
FROM synthetic_stocks
CROSS JOIN stock_group;

WITH trading_days AS (
    SELECT
        day::date AS bar_date,
        row_number() OVER (ORDER BY day)::integer AS day_number
    FROM generate_series(
        (current_date - INTERVAL '3 years')::date::timestamp,
        current_date::timestamp,
        INTERVAL '1 day'
    ) AS days(day)
    WHERE extract(isodow FROM day) BETWEEN 1 AND 5
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
    synthetic_stocks.id,
    '1d',
    trading_days.bar_date::timestamp AT TIME ZONE 'UTC',
    price.close - 0.1,
    price.close + 0.2,
    price.close - 0.3,
    price.close,
    1000 + synthetic_stocks.n * 17 + trading_days.day_number,
    1
FROM synthetic_stocks
CROSS JOIN trading_days
CROSS JOIN LATERAL (
    SELECT (50 + synthetic_stocks.n % 300 + trading_days.day_number % 20 / 10.0)::numeric AS close
) AS price;

WITH trading_days AS (
    SELECT day::date AS bar_date
    FROM generate_series(
        (current_date - INTERVAL '90 days')::date::timestamp,
        current_date::timestamp,
        INTERVAL '1 day'
    ) AS days(day)
    WHERE extract(isodow FROM day) BETWEEN 1 AND 5
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
    synthetic_stocks.id,
    '1h',
    (trading_days.bar_date + TIME '09:00' + make_interval(hours => hourly_bars.hour)) AT TIME ZONE 'UTC',
    price.close - 0.1,
    price.close + 0.2,
    price.close - 0.3,
    price.close,
    1000 + synthetic_stocks.n * 17 + hourly_bars.hour,
    1
FROM synthetic_stocks
JOIN trading_days ON synthetic_stocks.n <= 300
CROSS JOIN LATERAL generate_series(0, 6) AS hourly_bars(hour)
CROSS JOIN LATERAL (
    SELECT (50 + synthetic_stocks.n % 300 + hourly_bars.hour / 10.0)::numeric AS close
) AS price;

WITH trading_days AS (
    SELECT day::date AS bar_date
    FROM generate_series(
        (current_date - INTERVAL '14 days')::date::timestamp,
        current_date::timestamp,
        INTERVAL '1 day'
    ) AS days(day)
    WHERE extract(isodow FROM day) BETWEEN 1 AND 5
    ORDER BY day DESC
    LIMIT 10
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
    synthetic_stocks.id,
    '1m',
    (trading_days.bar_date + TIME '09:00' + make_interval(mins => minute_bars.minute)) AT TIME ZONE 'UTC',
    price.close - 0.1,
    price.close + 0.2,
    price.close - 0.3,
    price.close,
    1000 + synthetic_stocks.n * 17 + minute_bars.minute,
    1
FROM synthetic_stocks
JOIN trading_days ON synthetic_stocks.n <= 25
CROSS JOIN LATERAL generate_series(0, 389) AS minute_bars(minute)
CROSS JOIN LATERAL (
    SELECT (50 + synthetic_stocks.n % 300 + minute_bars.minute % 20 / 10.0)::numeric AS close
) AS price;

INSERT INTO valuation (code, date, mkt_cap)
SELECT synthetic_stocks.id || '0', current_date, 1000 + synthetic_stocks.n * 10
FROM synthetic_stocks;

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
