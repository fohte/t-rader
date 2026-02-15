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
