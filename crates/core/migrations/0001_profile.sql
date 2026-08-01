-- The singleton Profile: rendercv's `cv:` header, every field optional.
CREATE TABLE profile (
    id                 INTEGER PRIMARY KEY CHECK (id = 1),
    name               TEXT,
    headline           TEXT,
    location           TEXT,
    photo              TEXT,
    email              TEXT,
    phone              TEXT,
    website            TEXT,
    social_networks    TEXT,
    custom_connections TEXT
) STRICT;
