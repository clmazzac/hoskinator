-- The words an Entry is findable by. The store writes it; SQL cannot compute it.
ALTER TABLE entry ADD COLUMN search_text TEXT NOT NULL DEFAULT '';

-- One index over the whole Master Store, so one MATCH gives one ranked list.
CREATE VIRTUAL TABLE search_fts USING fts5(
    kind UNINDEXED,
    ref  UNINDEXED,
    body
);

CREATE TRIGGER search_fts_entry_after_insert
AFTER INSERT ON entry BEGIN
    INSERT INTO search_fts (kind, ref, body) VALUES ('entry', new.id, new.search_text);
END;

CREATE TRIGGER search_fts_entry_after_update
AFTER UPDATE ON entry BEGIN
    DELETE FROM search_fts WHERE kind = 'entry' AND ref = old.id;
    INSERT INTO search_fts (kind, ref, body) VALUES ('entry', new.id, new.search_text);
END;

CREATE TRIGGER search_fts_entry_after_delete
AFTER DELETE ON entry BEGIN
    DELETE FROM search_fts WHERE kind = 'entry' AND ref = old.id;
END;

CREATE TRIGGER search_fts_variant_after_insert
AFTER INSERT ON variant BEGIN
    INSERT INTO search_fts (kind, ref, body) VALUES ('variant', new.id, new.text);
END;

CREATE TRIGGER search_fts_variant_after_update
AFTER UPDATE ON variant BEGIN
    DELETE FROM search_fts WHERE kind = 'variant' AND ref = old.id;
    INSERT INTO search_fts (kind, ref, body) VALUES ('variant', new.id, new.text);
END;

CREATE TRIGGER search_fts_variant_after_delete
AFTER DELETE ON variant BEGIN
    DELETE FROM search_fts WHERE kind = 'variant' AND ref = old.id;
END;

-- Variants that already exist. Entries are backfilled on open, once their text can be computed.
INSERT INTO search_fts (kind, ref, body)
SELECT 'variant', id, text FROM variant;
