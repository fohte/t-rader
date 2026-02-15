CREATE TABLE watchlists (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
