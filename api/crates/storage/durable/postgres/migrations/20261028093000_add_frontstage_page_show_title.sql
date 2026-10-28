-- Preserve existing pages; future pages start without a default title bar.
ALTER TABLE frontstage_pages ADD COLUMN show_title boolean NOT NULL DEFAULT true;
ALTER TABLE frontstage_pages ALTER COLUMN show_title SET DEFAULT false;
