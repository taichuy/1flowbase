CREATE TABLE application_template_releases (
    workspace_id uuid NOT NULL,
    template_id text NOT NULL,
    release_version bigint NOT NULL CHECK (release_version > 0),
    checksum text NOT NULL,
    successful boolean NOT NULL DEFAULT false,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, template_id, release_version)
);
