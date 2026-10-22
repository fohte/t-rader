CREATE TEMP TABLE synthetic_reference_date AS
SELECT timezone('UTC', transaction_timestamp())::date AS reference_date;

CREATE TABLE query_perf_seed_metadata (
    profile_id text PRIMARY KEY,
    reference_date date NOT NULL
);

INSERT INTO query_perf_seed_metadata (profile_id, reference_date)
SELECT 'synthetic-market-v1', reference_date
FROM synthetic_reference_date;

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
    (SELECT reference_date - INTERVAL '12 years' FROM synthetic_reference_date)::timestamp,
    (SELECT reference_date FROM synthetic_reference_date)::timestamp,
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

CREATE TEMP TABLE synthetic_range_candidates AS
SELECT member.stock_id
FROM stock_group_member AS member
JOIN stock_group AS group_row ON group_row.id = member.group_id
JOIN group_axis AS axis ON axis.id = group_row.axis_id
WHERE axis.key = 'demo-axis'
  AND group_row.key = 'group-alpha';

CREATE TEMP TABLE synthetic_candidate_valuation_dates AS
SELECT bar_date,
       row_number() OVER (ORDER BY bar_date)::integer AS day_number
FROM (
    SELECT bar_date
    FROM synthetic_trading_days
    ORDER BY bar_date DESC
    LIMIT 400
) AS recent_trading_days;

CREATE TEMP TABLE synthetic_hourly_trading_days AS
SELECT day::date AS bar_date
FROM synthetic_reference_date AS reference
CROSS JOIN LATERAL generate_series(
    (reference.reference_date - INTERVAL '90 days')::timestamp,
    reference.reference_date::timestamp,
    INTERVAL '1 day'
) AS days(day)
WHERE extract(isodow FROM day) BETWEEN 1 AND 5;

CREATE TEMP TABLE synthetic_minute_trading_days AS
SELECT day::date AS bar_date
FROM synthetic_reference_date AS reference
CROSS JOIN LATERAL generate_series(
    (reference.reference_date - INTERVAL '14 days')::timestamp,
    reference.reference_date::timestamp,
    INTERVAL '1 day'
) AS days(day)
WHERE extract(isodow FROM day) BETWEEN 1 AND 5
ORDER BY day DESC
LIMIT 10;

SELECT format(
$daily$
INSERT INTO bars (
    instrument_id, timeframe, timestamp, open, high, low, close, volume, adjustment_factor
)
SELECT
    synthetic_stocks.id,
    '1d',
    synthetic_trading_days.bar_date::timestamp AT TIME ZONE 'UTC',
    price.close - 0.1,
    price.close + 0.2,
    price.close - 0.3,
    price.close,
    1000 + synthetic_stocks.n * 17 + synthetic_trading_days.day_number,
    1
FROM synthetic_stocks
CROSS JOIN synthetic_trading_days
CROSS JOIN LATERAL (
    SELECT (50 + synthetic_stocks.n %% 300 + synthetic_trading_days.day_number %% 20 / 10.0)::numeric AS close
) AS price
WHERE synthetic_trading_days.bar_date >= %L::date
  AND synthetic_trading_days.bar_date < %L::date
$daily$,
    batch.month_start,
    (batch.month_start + INTERVAL '1 month')::date
)
FROM (
    SELECT DISTINCT date_trunc('month', bar_date)::date AS month_start
    FROM synthetic_trading_days
) AS batch
ORDER BY batch.month_start
\gexec

INSERT INTO bars (
    instrument_id, timeframe, timestamp, open, high, low, close, volume, adjustment_factor
)
SELECT
    synthetic_stocks.id,
    '1h',
    (synthetic_hourly_trading_days.bar_date + TIME '09:00' + make_interval(hours => hourly_bars.hour)) AT TIME ZONE 'UTC',
    price.close - 0.1,
    price.close + 0.2,
    price.close - 0.3,
    price.close,
    1000 + synthetic_stocks.n * 17 + hourly_bars.hour,
    1
FROM synthetic_stocks
JOIN synthetic_hourly_trading_days ON synthetic_stocks.n <= 300
CROSS JOIN LATERAL generate_series(0, 6) AS hourly_bars(hour)
CROSS JOIN LATERAL (
    SELECT (50 + synthetic_stocks.n % 300 + hourly_bars.hour % 20 / 10.0)::numeric AS close
) AS price;

SELECT format(
$minute$
INSERT INTO bars (
    instrument_id, timeframe, timestamp, open, high, low, close, volume, adjustment_factor
)
SELECT
    synthetic_stocks.id,
    '1m',
    (synthetic_minute_trading_days.bar_date + TIME '09:00' + make_interval(mins => minute_bars.minute)) AT TIME ZONE 'UTC',
    price.close - 0.1,
    price.close + 0.2,
    price.close - 0.3,
    price.close,
    1000 + synthetic_stocks.n * 17 + minute_bars.minute,
    1
FROM synthetic_stocks
JOIN synthetic_minute_trading_days ON synthetic_stocks.n <= 3000
CROSS JOIN LATERAL generate_series(0, 389) AS minute_bars(minute)
CROSS JOIN LATERAL (
    SELECT (50 + synthetic_stocks.n %% 300 + minute_bars.minute %% 20 / 10.0)::numeric AS close
) AS price
WHERE synthetic_minute_trading_days.bar_date = %L::date
$minute$,
    synthetic_minute_trading_days.bar_date
)
FROM synthetic_minute_trading_days
ORDER BY synthetic_minute_trading_days.bar_date
\gexec

CREATE TEMP TABLE synthetic_bar_profile AS
SELECT
    instrument_id,
    timeframe,
    MIN(timestamp) AS earliest_timestamp,
    MAX(timestamp) AS latest_timestamp,
    COUNT(*)::bigint AS row_count
FROM bars
GROUP BY instrument_id, timeframe;

INSERT INTO bars_timestamp_coverage (
    instrument_id,
    timeframe,
    earliest_timestamp,
    latest_timestamp
)
SELECT instrument_id, timeframe, earliest_timestamp, latest_timestamp
FROM synthetic_bar_profile;

CREATE TEMP TABLE synthetic_valuation_dates AS
SELECT valuation_date.bar_date,
       row_number() OVER (ORDER BY valuation_date.bar_date)::integer AS month_number
FROM (
    SELECT DISTINCT ON (date_trunc('month', bar_date)) bar_date
    FROM synthetic_trading_days
    WHERE bar_date >= (SELECT reference_date - INTERVAL '2 years' FROM synthetic_reference_date)
      AND bar_date < (SELECT reference_date - INTERVAL '30 days' FROM synthetic_reference_date)
    ORDER BY date_trunc('month', bar_date), bar_date
) AS valuation_date;

INSERT INTO valuation (code, date, mkt_cap, per, pbr)
SELECT synthetic_stocks.id || '0',
       synthetic_valuation_dates.bar_date,
       1000 + synthetic_stocks.n * 10 + synthetic_valuation_dates.month_number,
       8 + synthetic_stocks.n % 32 + synthetic_valuation_dates.month_number % 3,
       1 + synthetic_stocks.n % 6 + synthetic_valuation_dates.month_number % 2
FROM synthetic_stocks
CROSS JOIN synthetic_valuation_dates
WHERE NOT EXISTS (
    SELECT FROM synthetic_range_candidates
    WHERE synthetic_range_candidates.stock_id = synthetic_stocks.id
);

INSERT INTO valuation (code, date, mkt_cap, per, pbr)
SELECT synthetic_stocks.id || '0',
       synthetic_candidate_valuation_dates.bar_date,
       1000 + synthetic_stocks.n * 10 + synthetic_candidate_valuation_dates.day_number,
       8 + synthetic_stocks.n % 32 + synthetic_candidate_valuation_dates.day_number % 3,
       1 + synthetic_stocks.n % 6 + synthetic_candidate_valuation_dates.day_number % 2
FROM synthetic_range_candidates
JOIN synthetic_stocks ON synthetic_stocks.id = synthetic_range_candidates.stock_id
CROSS JOIN synthetic_candidate_valuation_dates;

WITH valuation_days AS (
    SELECT bar_date
    FROM synthetic_trading_days
    ORDER BY bar_date DESC
    LIMIT 150
)
INSERT INTO valuation (code, date, mkt_cap)
SELECT synthetic_stocks.id || '0', valuation_days.bar_date, 1000 + synthetic_stocks.n * 10
FROM synthetic_stocks
CROSS JOIN valuation_days
ON CONFLICT (code, date) DO NOTHING;

INSERT INTO indicator (id, name, kind)
VALUES ('demo-indicator', 'Demo Indicator', 'sample-kind');

INSERT INTO indicator_observation (indicator_id, date, value)
SELECT 'demo-indicator', bar_date, 10 + day_number % 25
FROM synthetic_trading_days;

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
ANALYZE indicator;
ANALYZE indicator_observation;
