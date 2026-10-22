ALTER TABLE traces ADD COLUMN application TEXT;
UPDATE traces SET application = 'Not Specified' WHERE application IS NULL;
ALTER TABLE traces ALTER COLUMN application SET NOT NULL;