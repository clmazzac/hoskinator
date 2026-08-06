-- Bullets: one accomplishment inside an Entry, worded by one or more Variants.
CREATE TABLE bullet (
    id       INTEGER PRIMARY KEY,
    entry_id INTEGER NOT NULL REFERENCES entry (id) ON DELETE CASCADE,
    position INTEGER NOT NULL
) STRICT;

CREATE INDEX bullet_by_entry ON bullet (entry_id, position);

CREATE TABLE variant (
    id         INTEGER PRIMARY KEY,
    bullet_id  INTEGER NOT NULL REFERENCES bullet (id) ON DELETE CASCADE,
    text       TEXT NOT NULL,
    note       TEXT,
    is_default INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX variant_by_bullet ON variant (bullet_id, id);

-- A Bullet has at most one default. Having at least one is the store's to enforce.
CREATE UNIQUE INDEX variant_one_default ON variant (bullet_id) WHERE is_default = 1;

-- Highlights an Entry already holds become Bullets, in the order they were written.
INSERT INTO bullet (entry_id, position)
SELECT entry.id, highlight.key
FROM entry, json_each(entry.fields, '$.highlights') AS highlight
WHERE json_valid(entry.fields)
  AND json_type(entry.fields, '$.highlights') = 'array';

INSERT INTO variant (bullet_id, text, is_default)
SELECT bullet.id, highlight.value, 1
FROM bullet
JOIN entry ON entry.id = bullet.entry_id
JOIN json_each(entry.fields, '$.highlights') AS highlight ON highlight.key = bullet.position;

-- `highlights` is no longer a field an Entry carries.
UPDATE entry
SET fields = json_remove(fields, '$.highlights')
WHERE json_valid(fields)
  AND json_type(fields, '$.highlights') IS NOT NULL;
