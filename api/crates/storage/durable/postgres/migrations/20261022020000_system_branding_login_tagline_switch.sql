-- The login brand title is fixed; only the tagline area's visibility is configurable.
-- Preserve the configured logo, system name, description and filing HTML.
ALTER TABLE system_branding
    ADD COLUMN hide_login_tagline BOOLEAN NOT NULL DEFAULT FALSE,
    DROP COLUMN login_title;
