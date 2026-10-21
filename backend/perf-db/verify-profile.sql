\set ON_ERROR_STOP on
SET TIME ZONE 'UTC';

\if :record_profile
CREATE TABLE query_perf_profile_validation (
    profile_id text PRIMARY KEY,
    schema_id text NOT NULL,
    reference_date date NOT NULL,
    validation_status text NOT NULL CHECK (validation_status = 'passed'),
    validated_at timestamptz NOT NULL,
    runtime_metrics jsonb NOT NULL,
    build_metrics jsonb NOT NULL
);
\endif

CREATE TEMP TABLE query_perf_profile_current ON COMMIT PRESERVE ROWS AS
WITH seed AS (
    SELECT
        COUNT(*)::bigint AS metadata_rows,
        MIN(profile_id) AS profile_id,
        MIN(reference_date) AS reference_date
    FROM query_perf_seed_metadata
),
-- 基準日から期待範囲を独立に導出し、seed が作ったデータの不足を検出する。
expected_daily_dates AS (
    SELECT day::date AS bar_date
    FROM seed
    CROSS JOIN LATERAL generate_series(
        (seed.reference_date - INTERVAL '12 years')::timestamp,
        seed.reference_date::timestamp,
        INTERVAL '1 day'
    ) AS days(day)
    WHERE extract(isodow FROM day) BETWEEN 1 AND 5
),
expected_daily AS (
    SELECT MIN(bar_date) AS first_date, MAX(bar_date) AS last_date, COUNT(*)::bigint AS trading_days
    FROM expected_daily_dates
),
expected_hourly_days AS (
    SELECT day::date AS bar_date
    FROM seed
    CROSS JOIN LATERAL generate_series(
        (seed.reference_date - INTERVAL '90 days')::timestamp,
        seed.reference_date::timestamp,
        INTERVAL '1 day'
    ) AS days(day)
    WHERE extract(isodow FROM day) BETWEEN 1 AND 5
),
expected_hourly AS (
    SELECT MIN(bar_date) AS first_date, MAX(bar_date) AS last_date, COUNT(*)::bigint AS trading_days
    FROM expected_hourly_days
),
expected_minute_days AS (
    SELECT day::date AS bar_date
    FROM seed
    CROSS JOIN LATERAL generate_series(
        (seed.reference_date - INTERVAL '14 days')::timestamp,
        seed.reference_date::timestamp,
        INTERVAL '1 day'
    ) AS days(day)
    WHERE extract(isodow FROM day) BETWEEN 1 AND 5
    ORDER BY day DESC
    LIMIT 10
),
expected_minute AS (
    SELECT MIN(bar_date) AS first_date, MAX(bar_date) AS last_date, COUNT(*)::bigint AS trading_days
    FROM expected_minute_days
),
expected_valuation_days AS (
    SELECT bar_date
    FROM expected_daily_dates
    ORDER BY bar_date DESC
    LIMIT 150
),
expected_valuation AS (
    SELECT MIN(bar_date) AS first_date, MAX(bar_date) AS last_date, COUNT(*)::bigint AS trading_days
    FROM expected_valuation_days
),
schema_metrics AS (
    SELECT
        COUNT(*)::bigint AS migration_count,
        md5(coalesce(string_agg(version, ',' ORDER BY version), '')) AS schema_id
    FROM seaql_migrations
),
coverage AS (
    SELECT
        COUNT(*)::bigint AS row_count,
        md5(coalesce(
            string_agg(
                concat_ws('|', instrument_id, timeframe, earliest_timestamp::text, latest_timestamp::text),
                E'\n' ORDER BY instrument_id, timeframe
            ),
            ''
        )) AS fingerprint
    FROM bars_timestamp_coverage
),
coverage_by_timeframe AS (
    SELECT
        timeframe,
        COUNT(*)::bigint AS instrument_count,
        MIN((earliest_timestamp AT TIME ZONE 'UTC')::date) AS first_date,
        MAX((latest_timestamp AT TIME ZONE 'UTC')::date) AS last_date
    FROM bars_timestamp_coverage
    GROUP BY timeframe
),
minute_profiles AS (
    SELECT
        instrument_id,
        (timestamp AT TIME ZONE 'UTC')::date AS bar_date,
        COUNT(*)::bigint AS row_count,
        COUNT(*) FILTER (
            WHERE (timestamp AT TIME ZONE 'UTC')::time BETWEEN TIME '09:00' AND TIME '15:29'
        ) AS session_row_count,
        MIN((timestamp AT TIME ZONE 'UTC')::time) AS first_time,
        MAX((timestamp AT TIME ZONE 'UTC')::time) AS last_time
    FROM bars
    WHERE timeframe = '1m'
    GROUP BY instrument_id, (timestamp AT TIME ZONE 'UTC')::date
),
minute_profile_metrics AS (
    SELECT
        COUNT(*)::bigint AS profile_count,
        COUNT(*) FILTER (
            WHERE row_count = 390
              AND session_row_count = 390
              AND first_time = TIME '09:00'
              AND last_time = TIME '15:29'
        )::bigint AS complete_profile_count,
        COUNT(DISTINCT instrument_id)::bigint AS instrument_count,
        COUNT(DISTINCT bar_date)::bigint AS trading_day_count,
        MIN(bar_date) AS first_date,
        MAX(bar_date) AS last_date
    FROM minute_profiles
),
latest_chunk AS (
    SELECT range_start, range_end
    FROM timescaledb_information.chunks
    WHERE hypertable_schema = 'public'
      AND hypertable_name = 'bars'
      AND NOT is_compressed
    ORDER BY range_start DESC
    LIMIT 1
),
latest_chunk_metrics AS (
    SELECT
        (SELECT range_start FROM latest_chunk) AS range_start,
        (SELECT range_end FROM latest_chunk) AS range_end,
        (
            SELECT COUNT(*)::bigint
            FROM bars
            JOIN latest_chunk ON bars.timestamp >= latest_chunk.range_start AND bars.timestamp < latest_chunk.range_end
        ) AS row_count,
        (
            SELECT COUNT(*)::bigint
            FROM bars
            JOIN latest_chunk ON bars.timestamp >= latest_chunk.range_start AND bars.timestamp < latest_chunk.range_end
            WHERE bars.timeframe = '1m'
        ) AS minute_row_count
),
valuation_metrics AS (
    SELECT
        COUNT(*)::bigint AS row_count,
        COUNT(DISTINCT date)::bigint AS date_count,
        MIN(date) AS first_date,
        MAX(date) AS last_date
    FROM valuation
),
groups_by_axis AS (
    SELECT
        group_axis.id AS axis_id,
        stock_group.id AS group_id,
        COUNT(stock_group_member.stock_id)::bigint AS member_count
    FROM group_axis
    JOIN stock_group ON stock_group.axis_id = group_axis.id
    LEFT JOIN stock_group_member ON stock_group_member.group_id = stock_group.id
    WHERE group_axis.key IN ('sample-axis', 'demo-axis', 'other-axis')
    GROUP BY group_axis.id, stock_group.id
),
group_metrics AS (
    SELECT
        COUNT(DISTINCT axis_id)::bigint AS axis_count,
        COUNT(*)::bigint AS group_count,
        coalesce(SUM(member_count), 0)::bigint AS member_count,
        coalesce(MIN(member_count), 0)::bigint AS min_members,
        coalesce(MAX(member_count), 0)::bigint AS max_members
    FROM groups_by_axis
)
SELECT jsonb_build_object(
    'profile_id', seed.profile_id,
    'metadata_rows', seed.metadata_rows,
    'reference_date', seed.reference_date,
    'schema_id', schema_metrics.schema_id,
    'migration_count', schema_metrics.migration_count,
    'timescaledb_max_background_workers', current_setting('timescaledb.max_background_workers')::integer,
    'stock_count', (SELECT COUNT(*)::bigint FROM stock),
    'instrument_count', (SELECT COUNT(*)::bigint FROM instruments),
    'bar_chunk_count', (
        SELECT COUNT(*)::bigint
        FROM timescaledb_information.chunks
        WHERE hypertable_schema = 'public'
          AND hypertable_name = 'bars'
    ),
    'coverage_row_count', coverage.row_count,
    'coverage_fingerprint', coverage.fingerprint,
    'daily_instrument_count', coalesce(daily.instrument_count, 0),
    'daily_first_date', daily.first_date,
    'daily_last_date', daily.last_date,
    'expected_daily_first_date', expected_daily.first_date,
    'expected_daily_last_date', expected_daily.last_date,
    'expected_daily_trading_days', expected_daily.trading_days,
    'hourly_instrument_count', coalesce(hourly.instrument_count, 0),
    'hourly_first_date', hourly.first_date,
    'hourly_last_date', hourly.last_date,
    'expected_hourly_first_date', expected_hourly.first_date,
    'expected_hourly_last_date', expected_hourly.last_date,
    'expected_hourly_trading_days', expected_hourly.trading_days,
    'minute_instrument_count', coalesce(minute.instrument_count, 0),
    'minute_first_date', minute.first_date,
    'minute_last_date', minute.last_date,
    'expected_minute_first_date', expected_minute.first_date,
    'expected_minute_last_date', expected_minute.last_date,
    'expected_minute_trading_days', expected_minute.trading_days,
    'minute_profile_count', minute_profile_metrics.profile_count,
    'minute_complete_profile_count', minute_profile_metrics.complete_profile_count,
    'minute_profile_instrument_count', minute_profile_metrics.instrument_count,
    'minute_trading_day_count', minute_profile_metrics.trading_day_count
) || jsonb_build_object(
    'minute_profile_first_date', minute_profile_metrics.first_date,
    'minute_profile_last_date', minute_profile_metrics.last_date,
    'valuation_row_count', valuation_metrics.row_count,
    'valuation_date_count', valuation_metrics.date_count,
    'valuation_first_date', valuation_metrics.first_date,
    'valuation_last_date', valuation_metrics.last_date,
    'expected_valuation_first_date', expected_valuation.first_date,
    'expected_valuation_last_date', expected_valuation.last_date,
    'expected_valuation_trading_days', expected_valuation.trading_days,
    'group_axis_count', group_metrics.axis_count,
    'group_count', group_metrics.group_count,
    'group_member_count', group_metrics.member_count,
    'min_group_members', group_metrics.min_members,
    'max_group_members', group_metrics.max_members,
    'latest_chunk_start', latest_chunk_metrics.range_start,
    'latest_chunk_end', latest_chunk_metrics.range_end,
    'latest_chunk_row_count', latest_chunk_metrics.row_count,
    'latest_chunk_minute_row_count', latest_chunk_metrics.minute_row_count
) AS metrics
FROM seed
CROSS JOIN expected_daily
CROSS JOIN expected_hourly
CROSS JOIN expected_minute
CROSS JOIN expected_valuation
CROSS JOIN schema_metrics
CROSS JOIN coverage
CROSS JOIN valuation_metrics
CROSS JOIN minute_profile_metrics
CROSS JOIN group_metrics
CROSS JOIN latest_chunk_metrics
LEFT JOIN coverage_by_timeframe AS daily ON daily.timeframe = '1d'
LEFT JOIN coverage_by_timeframe AS hourly ON hourly.timeframe = '1h'
LEFT JOIN coverage_by_timeframe AS minute ON minute.timeframe = '1m';

DO $$
DECLARE
    metrics jsonb;
BEGIN
    SELECT profile_current.metrics
    INTO STRICT metrics
    FROM query_perf_profile_current AS profile_current;

    IF metrics->>'profile_id' IS DISTINCT FROM 'synthetic-market-v1' OR metrics->>'metadata_rows' IS DISTINCT FROM '1' THEN
        RAISE EXCEPTION 'synthetic profile metadata is invalid: profile_id=%, rows=%', metrics->>'profile_id', metrics->>'metadata_rows';
    END IF;
    IF metrics->>'schema_id' IS NULL OR metrics->>'migration_count' IS NULL OR (metrics->>'migration_count')::bigint = 0 THEN
        RAISE EXCEPTION 'synthetic profile schema identifier is missing';
    END IF;
    IF (metrics->>'timescaledb_max_background_workers')::integer <> 0 THEN
        RAISE EXCEPTION 'synthetic profile requires TimescaleDB background workers to stay disabled';
    END IF;
    IF (metrics->>'stock_count')::bigint <> 15000 OR (metrics->>'instrument_count')::bigint <> 15000 THEN
        RAISE EXCEPTION 'synthetic profile requires 15,000 stocks and instruments: stocks=%, instruments=%', metrics->>'stock_count', metrics->>'instrument_count';
    END IF;
    IF (metrics->>'bar_chunk_count')::bigint < 600 THEN
        RAISE EXCEPTION 'synthetic profile requires at least 600 bars chunks: found %', metrics->>'bar_chunk_count';
    END IF;
    IF (metrics->>'coverage_row_count')::bigint <> 18300 THEN
        RAISE EXCEPTION 'synthetic profile requires 18,300 bars timestamp summaries: found %', metrics->>'coverage_row_count';
    END IF;
    IF (metrics->>'daily_instrument_count')::bigint <> 15000
        OR metrics->>'daily_first_date' IS DISTINCT FROM metrics->>'expected_daily_first_date'
        OR metrics->>'daily_last_date' IS DISTINCT FROM metrics->>'expected_daily_last_date' THEN
        RAISE EXCEPTION 'daily bars do not cover the expected 12-year range: actual=%..%, expected=%..%',
            metrics->>'daily_first_date', metrics->>'daily_last_date',
            metrics->>'expected_daily_first_date', metrics->>'expected_daily_last_date';
    END IF;
    IF (metrics->>'hourly_instrument_count')::bigint <> 300
        OR metrics->>'hourly_first_date' IS DISTINCT FROM metrics->>'expected_hourly_first_date'
        OR metrics->>'hourly_last_date' IS DISTINCT FROM metrics->>'expected_hourly_last_date' THEN
        RAISE EXCEPTION 'hourly bars do not cover the expected 90-day range: actual=%..%, expected=%..%',
            metrics->>'hourly_first_date', metrics->>'hourly_last_date',
            metrics->>'expected_hourly_first_date', metrics->>'expected_hourly_last_date';
    END IF;
    IF (metrics->>'minute_instrument_count')::bigint <> 3000
        OR metrics->>'minute_first_date' IS DISTINCT FROM metrics->>'expected_minute_first_date'
        OR metrics->>'minute_last_date' IS DISTINCT FROM metrics->>'expected_minute_last_date' THEN
        RAISE EXCEPTION 'minute bars do not cover the expected 10 trading days: actual=%..%, expected=%..%',
            metrics->>'minute_first_date', metrics->>'minute_last_date',
            metrics->>'expected_minute_first_date', metrics->>'expected_minute_last_date';
    END IF;
    IF (metrics->>'minute_profile_count')::bigint <> 30000
        OR (metrics->>'minute_complete_profile_count')::bigint <> 30000
        OR (metrics->>'minute_profile_instrument_count')::bigint <> 3000
        OR (metrics->>'minute_trading_day_count')::bigint <> (metrics->>'expected_minute_trading_days')::bigint
        OR metrics->>'minute_profile_first_date' IS DISTINCT FROM metrics->>'expected_minute_first_date'
        OR metrics->>'minute_profile_last_date' IS DISTINCT FROM metrics->>'expected_minute_last_date' THEN
        RAISE EXCEPTION 'each minute instrument must have 390 bars from 09:00 through 15:29 on every expected trading day: profiles=%, complete=%, instruments=%, days=%',
            metrics->>'minute_profile_count', metrics->>'minute_complete_profile_count',
            metrics->>'minute_profile_instrument_count', metrics->>'minute_trading_day_count';
    END IF;
    IF (metrics->>'valuation_row_count')::bigint < 2000000
        OR (metrics->>'valuation_date_count')::bigint < 134
        OR metrics->>'valuation_first_date' IS DISTINCT FROM metrics->>'expected_valuation_first_date'
        OR metrics->>'valuation_last_date' IS DISTINCT FROM metrics->>'expected_valuation_last_date' THEN
        RAISE EXCEPTION 'valuation data does not cover at least two million rows and its expected date range: rows=%, dates=%',
            metrics->>'valuation_row_count', metrics->>'valuation_date_count';
    END IF;
    IF (metrics->>'group_axis_count')::bigint <> 3
        OR (metrics->>'group_count')::bigint <> 90
        OR (metrics->>'group_member_count')::bigint <> 45000
        OR (metrics->>'min_group_members')::bigint <> 500
        OR (metrics->>'max_group_members')::bigint <> 500 THEN
        RAISE EXCEPTION 'synthetic group distribution is incomplete: axes=%, groups=%, members=%, group_sizes=%..%',
            metrics->>'group_axis_count', metrics->>'group_count', metrics->>'group_member_count',
            metrics->>'min_group_members', metrics->>'max_group_members';
    END IF;
    IF metrics->>'latest_chunk_start' IS NULL
        OR (metrics->>'latest_chunk_row_count')::bigint = 0
        OR (metrics->>'latest_chunk_minute_row_count')::bigint * 100 < (metrics->>'latest_chunk_row_count')::bigint * 98
        OR (metrics->>'latest_chunk_minute_row_count')::bigint * 100 > (metrics->>'latest_chunk_row_count')::bigint * 99 THEN
        RAISE EXCEPTION 'latest uncompressed bars chunk minute ratio must be 98-99 percent: minute_rows=%, total_rows=%',
            metrics->>'latest_chunk_minute_row_count', metrics->>'latest_chunk_row_count';
    END IF;
END
$$;

\if :record_profile
DO $$
DECLARE
    metrics jsonb;
    expected_daily_rows bigint;
    expected_hourly_rows bigint;
    expected_minute_rows bigint;
    expected_valuation_rows bigint;
    daily_profile_rows bigint;
    hourly_profile_rows bigint;
    minute_profile_rows bigint;
    unexpected_profile_rows bigint;
    coverage_mismatches bigint;
    valuation_rows bigint;
    valuation_codes bigint;
    valuation_dates bigint;
    profile_metrics jsonb;
BEGIN
    SELECT profile_current.metrics
    INTO STRICT metrics
    FROM query_perf_profile_current AS profile_current;

    expected_daily_rows := (metrics->>'expected_daily_trading_days')::bigint;
    expected_hourly_rows := (metrics->>'expected_hourly_trading_days')::bigint * 7;
    expected_minute_rows := (metrics->>'expected_minute_trading_days')::bigint * 390;
    expected_valuation_rows := (metrics->>'expected_valuation_trading_days')::bigint;

    SELECT COUNT(*)::bigint INTO daily_profile_rows
    FROM synthetic_bar_profile
    WHERE timeframe = '1d'
      AND row_count = expected_daily_rows
      AND (earliest_timestamp AT TIME ZONE 'UTC')::date = (metrics->>'expected_daily_first_date')::date
      AND (latest_timestamp AT TIME ZONE 'UTC')::date = (metrics->>'expected_daily_last_date')::date;
    IF daily_profile_rows <> 15000 THEN
        RAISE EXCEPTION 'each synthetic stock must have complete 12-year daily bars: valid=% of 15,000', daily_profile_rows;
    END IF;

    SELECT COUNT(*)::bigint INTO hourly_profile_rows
    FROM synthetic_bar_profile
    WHERE timeframe = '1h'
      AND row_count = expected_hourly_rows
      AND (earliest_timestamp AT TIME ZONE 'UTC')::date = (metrics->>'expected_hourly_first_date')::date
      AND (latest_timestamp AT TIME ZONE 'UTC')::date = (metrics->>'expected_hourly_last_date')::date;
    IF hourly_profile_rows <> 300 THEN
        RAISE EXCEPTION 'each hourly instrument must have complete 90-day bars: valid=% of 300', hourly_profile_rows;
    END IF;

    SELECT COUNT(*)::bigint INTO minute_profile_rows
    FROM synthetic_bar_profile
    WHERE timeframe = '1m'
      AND row_count = expected_minute_rows
      AND (earliest_timestamp AT TIME ZONE 'UTC')::date = (metrics->>'expected_minute_first_date')::date
      AND (latest_timestamp AT TIME ZONE 'UTC')::date = (metrics->>'expected_minute_last_date')::date;
    IF minute_profile_rows <> 3000 THEN
        RAISE EXCEPTION 'each minute instrument must have complete recent bars: valid=% of 3,000', minute_profile_rows;
    END IF;

    SELECT COUNT(*)::bigint INTO unexpected_profile_rows
    FROM synthetic_bar_profile
    WHERE timeframe NOT IN ('1d', '1h', '1m');
    IF unexpected_profile_rows <> 0
        OR (SELECT COUNT(*) FROM synthetic_bar_profile WHERE timeframe = '1d') <> 15000
        OR (SELECT COUNT(*) FROM synthetic_bar_profile WHERE timeframe = '1h') <> 300
        OR (SELECT COUNT(*) FROM synthetic_bar_profile WHERE timeframe = '1m') <> 3000 THEN
        RAISE EXCEPTION 'bars contain unexpected or missing instrument/timeframe summaries';
    END IF;

    SELECT COUNT(*)::bigint INTO coverage_mismatches
    FROM synthetic_bar_profile AS summary
    FULL OUTER JOIN bars_timestamp_coverage AS coverage
      USING (instrument_id, timeframe)
    WHERE summary.instrument_id IS NULL
       OR coverage.instrument_id IS NULL
       OR summary.earliest_timestamp IS DISTINCT FROM coverage.earliest_timestamp
       OR summary.latest_timestamp IS DISTINCT FROM coverage.latest_timestamp;
    IF coverage_mismatches <> 0 THEN
        RAISE EXCEPTION 'bars timestamp coverage differs from bars: mismatches=%', coverage_mismatches;
    END IF;

    SELECT COUNT(*)::bigint, COUNT(DISTINCT code)::bigint, COUNT(DISTINCT date)::bigint
    INTO valuation_rows, valuation_codes, valuation_dates
    FROM valuation;
    IF valuation_codes <> 15000 OR valuation_dates <> expected_valuation_rows
        OR valuation_rows <> valuation_codes * valuation_dates THEN
        RAISE EXCEPTION 'valuation history must cover each synthetic code on every expected date: rows=%, codes=%, dates=%',
            valuation_rows, valuation_codes, valuation_dates;
    END IF;

    SELECT jsonb_build_object(
        'coverage_mismatch_count', coverage_mismatches,
        'bar_profile_row_count', (SELECT COUNT(*)::bigint FROM synthetic_bar_profile),
        'bar_instruments_by_timeframe', (
            SELECT jsonb_object_agg(timeframe, instrument_count)
            FROM (
                SELECT timeframe, COUNT(*)::bigint AS instrument_count
                FROM synthetic_bar_profile
                GROUP BY timeframe
            ) AS counts
        ),
        'bar_rows_by_timeframe', (
            SELECT jsonb_object_agg(timeframe, row_count)
            FROM (
                SELECT timeframe, SUM(row_count)::bigint AS row_count
                FROM synthetic_bar_profile
                GROUP BY timeframe
            ) AS counts
        ),
        'valuation_code_count', valuation_codes,
        'valuation_date_count', valuation_dates,
        'valuation_row_count', valuation_rows
    )
    INTO profile_metrics;

    INSERT INTO query_perf_profile_validation (
        profile_id,
        schema_id,
        reference_date,
        validation_status,
        validated_at,
        runtime_metrics,
        build_metrics
    )
    SELECT
        metrics->>'profile_id',
        metrics->>'schema_id',
        (metrics->>'reference_date')::date,
        'passed',
        clock_timestamp(),
        metrics,
        profile_metrics
    FROM query_perf_profile_current
    ON CONFLICT (profile_id) DO UPDATE SET
        schema_id = EXCLUDED.schema_id,
        reference_date = EXCLUDED.reference_date,
        validation_status = EXCLUDED.validation_status,
        validated_at = EXCLUDED.validated_at,
        runtime_metrics = EXCLUDED.runtime_metrics,
        build_metrics = EXCLUDED.build_metrics;
END
$$;
\else
DO $$
DECLARE
    metrics jsonb;
    saved query_perf_profile_validation%ROWTYPE;
BEGIN
    SELECT profile_current.metrics
    INTO STRICT metrics
    FROM query_perf_profile_current AS profile_current;

    SELECT * INTO saved
    FROM query_perf_profile_validation
    WHERE profile_id = 'synthetic-market-v1';
    IF NOT FOUND THEN
        RAISE EXCEPTION 'saved synthetic profile validation result is missing';
    END IF;
    IF saved.validation_status IS DISTINCT FROM 'passed' OR saved.validated_at IS NULL THEN
        RAISE EXCEPTION 'saved synthetic profile validation result is not passing';
    END IF;
    IF saved.schema_id IS DISTINCT FROM metrics->>'schema_id'
        OR saved.reference_date IS DISTINCT FROM (metrics->>'reference_date')::date THEN
        RAISE EXCEPTION 'saved synthetic profile schema or reference date does not match the running database';
    END IF;
    IF saved.runtime_metrics IS DISTINCT FROM metrics THEN
        RAISE EXCEPTION 'live synthetic profile metrics differ from the saved validation result';
    END IF;
    IF saved.build_metrics->>'coverage_mismatch_count' IS DISTINCT FROM '0'
        OR saved.build_metrics->>'bar_profile_row_count' IS DISTINCT FROM '18300' THEN
        RAISE EXCEPTION 'saved synthetic profile build validation result is incomplete';
    END IF;
END
$$;
\endif

SELECT jsonb_pretty(metrics)
FROM query_perf_profile_current;
