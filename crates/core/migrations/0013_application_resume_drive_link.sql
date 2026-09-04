-- The Drive link for the resume that was actually sent — filled in by hand, since Hoskinator
-- has no view into Drive. Distinct from `resume_branch`, which names the version in the repo.
ALTER TABLE application ADD COLUMN resume_drive_link TEXT;
