-- Links a Job Description to the application it was pasted onto, when it came from one.
ALTER TABLE job_description ADD COLUMN application_id INTEGER REFERENCES application (id) ON DELETE CASCADE;

-- SQLite lets any number of NULLs through a UNIQUE index, so this caps it at one linked row per
-- application while leaving every unlinked (NULL) one alone.
CREATE UNIQUE INDEX job_description_by_application ON job_description (application_id);
