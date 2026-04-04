-- Optional password for account recovery / future OAuth linking (guest users stay password-less).
ALTER TABLE users ADD COLUMN password_hash TEXT;
