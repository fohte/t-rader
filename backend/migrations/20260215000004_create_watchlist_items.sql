CREATE TABLE watchlist_items (
    watchlist_id UUID NOT NULL REFERENCES watchlists(id) ON DELETE CASCADE,
    instrument_id TEXT NOT NULL REFERENCES instruments(id) ON DELETE CASCADE,
    sort_order INTEGER NOT NULL,
    added_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (watchlist_id, instrument_id)
);

CREATE INDEX idx_watchlist_items_watchlist_id ON watchlist_items (watchlist_id);
