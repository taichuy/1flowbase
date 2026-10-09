-- created_at is the source event time for imported records, not their arrival time.
-- A stable default backfills pre-existing rows without a volatile table rewrite.
ALTER TABLE application_run_log_tasks
    ADD COLUMN ingested_at timestamptz NOT NULL DEFAULT now();
-- For new rows capture wall-clock arrival after ingestion obtains its application lock.
ALTER TABLE application_run_log_tasks ALTER COLUMN ingested_at SET DEFAULT clock_timestamp();
