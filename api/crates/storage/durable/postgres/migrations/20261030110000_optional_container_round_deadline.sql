-- NULL means no round deadline. Existing finite deadlines remain unchanged.
ALTER TABLE flow_container_rounds ALTER COLUMN deadline_unix_ms DROP NOT NULL;
