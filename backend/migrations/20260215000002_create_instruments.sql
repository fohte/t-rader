CREATE TABLE instruments (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    market TEXT NOT NULL CHECK (market IN ('TSE')),
    sector TEXT
);
