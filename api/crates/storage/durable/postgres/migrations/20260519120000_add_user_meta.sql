ALTER TABLE users
ADD COLUMN meta jsonb NOT NULL DEFAULT '{}'::jsonb;

ALTER TABLE users
ADD CONSTRAINT users_meta_object_check
CHECK (jsonb_typeof(meta) = 'object');
