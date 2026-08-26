-- Scopes each application to the repository it was tracked against.
ALTER TABLE application ADD COLUMN repository TEXT NOT NULL DEFAULT '';

CREATE INDEX application_by_repository ON application (repository);
