-- Entries: rendercv's fields held as JSON, keyed by the entry type they belong to.
-- `date`, `start_date`, and `end_date` are copies of what `fields` holds.
CREATE TABLE entry (
    id         INTEGER PRIMARY KEY,
    entry_type TEXT NOT NULL,
    fields     TEXT NOT NULL,
    date       TEXT,
    start_date TEXT,
    end_date   TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE INDEX entry_by_type ON entry (entry_type);
