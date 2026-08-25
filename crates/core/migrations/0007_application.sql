-- Job applications, each optionally tied to the resume branch it was sent with.
CREATE TABLE application (
    id            INTEGER PRIMARY KEY,
    company       TEXT NOT NULL,
    position      TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'draft',
    date_applied  TEXT,
    listing_url   TEXT,
    resume_branch TEXT,
    notes         TEXT,
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE INDEX application_by_branch ON application (resume_branch);
