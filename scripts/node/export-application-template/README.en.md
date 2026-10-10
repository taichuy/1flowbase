# Export and update application templates

[简体中文](README.md)

Export definitions from a running application into the official plugin repository. The backend computes the dependency closure for pages, applications, data model definitions, and MCP. Business records, account passwords, and external connection credentials are excluded.

Create a selection file:

```json
{
  "page_ids": ["PAGE_UUID"],
  "application_ids": ["APPLICATION_UUID"],
  "data_model_ids": [],
  "mcp_instance_ids": ["1flowbase"],
  "i18n_keys": ["Reports", "Daily reports"]
}
```

Run from the main repository:

```sh
node scripts/node/export-application-template/cli.js \
  --target /home/taichuy/git/1flowbase-official-plugins/applications-demo/@taichuy/gateway-demo \
  --selection /absolute/selection.json \
  --name 'Gateway Demo' \
  --api-base-url http://127.0.0.1:7800
```

The first export saves the selection, source URL, name, and stable template identity in `export.config.json`. Subsequent exports can pass only `--target`; pass `--selection` again to change the selected resources. Content or metadata changes increment the release version; an unchanged export retains its version and timestamp.

`i18n_keys` selects source-message keys from the global translation catalog and exports their language entries. Translation-only selections are supported. Omitting this field does not select the entire catalog. Keys built dynamically in block code must be selected explicitly. The application-template installer decides whether each target entry can be updated or must preserve local changes.

Packages with translations use `1flowbase.portable-template/v2`; packages without translations retain v1. The file/checksum archive envelope remains `1flowbase.application-template-archive/v1`. Existing archives remain readable; unsupported package versions are rejected instead of silently dropping translations.

The CLI uses a temporary owner session and always releases it. Credentials come from the private main-repository `.env` and are never saved in the template. An isolated worktree can pass `--repo-root /home/taichuy/git/1flowbase` to use that repository's authentication configuration.

The export is written and validated in a temporary directory before atomic replacement. Failure preserves the previous template. Definitions are split into referenced files with checksums. `export.config.json`, README files, and catalog metadata are excluded from release ZIPs.

The CLI does not commit or push. Pushing the template source to the plugin repository's `main` branch triggers the application-template workflow, which signs and publishes immutable ZIP assets and refreshes the catalog. `scripts/application-template/archive.mjs` in that repository provides the shared source validator and local packager. Settings → Backups creates or uploads ZIPs through the same backend installation entry point.
