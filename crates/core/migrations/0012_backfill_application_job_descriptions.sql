-- An application that already carried a pasted posting before 0011 introduced the link needs its
-- Job Description backfilled too, same as job_description_fts was for job_description in 0003.
INSERT INTO job_description (application_id, title, text)
SELECT id, company || ' — ' || position, TRIM(jd_text)
FROM application
WHERE jd_text IS NOT NULL AND TRIM(jd_text) != '';
