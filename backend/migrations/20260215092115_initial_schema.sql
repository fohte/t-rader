CREATE EXTENSION IF NOT EXISTS timescaledb;

CREATE TABLE instruments (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    market TEXT NOT NULL CHECK (market IN ('TSE')),
    sector TEXT
);

CREATE TABLE watchlists (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE watchlist_items (
    watchlist_id UUID NOT NULL REFERENCES watchlists(id) ON DELETE CASCADE,
    instrument_id TEXT NOT NULL REFERENCES instruments(id) ON DELETE CASCADE,
    sort_order INTEGER NOT NULL,
    added_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (watchlist_id, instrument_id)
);

CREATE TABLE bars (
    instrument_id TEXT NOT NULL REFERENCES instruments(id) ON DELETE CASCADE,
    timeframe TEXT NOT NULL CHECK (timeframe IN ('1d')),
    timestamp TIMESTAMPTZ NOT NULL,
    open NUMERIC NOT NULL,
    high NUMERIC NOT NULL,
    low NUMERIC NOT NULL,
    close NUMERIC NOT NULL,
    volume BIGINT NOT NULL,
    PRIMARY KEY (instrument_id, timeframe, timestamp)
);

SELECT create_hypertable('bars', by_range('timestamp'));
