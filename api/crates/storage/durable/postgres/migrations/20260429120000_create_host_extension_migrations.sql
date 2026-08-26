CREATE TABLE IF NOT EXISTS host_extension_migrations (
    id UUID PRIMARY KEY,
    extension_id TEXT NOT NULL,
    plugin_version TEXT NOT NULL,
    migration_id TEXT NOT NULL,
    checksum TEXT NOT NULL,
    package_fingerprint TEXT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (extension_id, migration_id)
);
