-- Sections: the curated `{ name, entry_type }` vocabulary.
-- `entry_type` is unconstrained here; `EntryType` is the one authority.
CREATE TABLE section (
    name       TEXT PRIMARY KEY,
    entry_type TEXT NOT NULL
) STRICT;
