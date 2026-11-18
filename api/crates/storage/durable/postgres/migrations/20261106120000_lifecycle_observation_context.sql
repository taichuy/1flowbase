-- Trusted source-owner metadata only. Existing canonical payload remains the sole body.
-- Historical NULL rows deliberately remain unavailable for source workflow delivery.
ALTER TABLE lifecycle_outbox ADD COLUMN observation_context jsonb;
