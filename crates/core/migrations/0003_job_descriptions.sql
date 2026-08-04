-- Standalone Job Descriptions and their full-text index.
CREATE TABLE job_description (
    id         INTEGER PRIMARY KEY,
    title      TEXT,
    text       TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE VIRTUAL TABLE job_description_fts USING fts5(
    title,
    text,
    content = 'job_description',
    content_rowid = 'id'
);

CREATE TRIGGER job_description_fts_after_insert
AFTER INSERT ON job_description BEGIN
    INSERT INTO job_description_fts (rowid, title, text)
    VALUES (new.id, new.title, new.text);
END;

CREATE TRIGGER job_description_fts_after_update
AFTER UPDATE ON job_description BEGIN
    INSERT INTO job_description_fts (job_description_fts, rowid, title, text)
    VALUES ('delete', old.id, old.title, old.text);
    INSERT INTO job_description_fts (rowid, title, text)
    VALUES (new.id, new.title, new.text);
END;

CREATE TRIGGER job_description_fts_after_delete
AFTER DELETE ON job_description BEGIN
    INSERT INTO job_description_fts (job_description_fts, rowid, title, text)
    VALUES ('delete', old.id, old.title, old.text);
END;

-- Required for databases that already contain postings when this index is introduced.
INSERT INTO job_description_fts (rowid, title, text)
SELECT id, title, text FROM job_description;
