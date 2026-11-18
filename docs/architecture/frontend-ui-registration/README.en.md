# Frontend UI Registration

Execution authority and UI registration targets are separate. `runtime: native_react` identifies the execution surface; it does not opt a contribution into page creation.

| Manifest declaration | Host data | Consumer |
| --- | --- | --- |
| `ui_registration: component` | `frontend_block_catalog` and plugin-owned `ui_component_records` | Component management and editor resources |
| `ui_registration: code_template` | Official-template projection from `frontend_block_catalog`; manually managed templates and revisions use `ui_code_templates` | Code templates and editor resources |
| `block_registration: true` | `frontend_block_catalog.block_registration` | Block management and the create-block menu |

Host 0.6.3 adds `block_registration`, defaulting to `false`. Contributions intended for direct page creation must explicitly set it to `true`. `ui_registration` selects either component or template management; `block_registration` independently enables direct page creation. To provide both components and templates, declare distinct contribution codes and share `code_modules` where appropriate. Mounting source does not register additional UI resources.

```yaml
block_contributions:
  - contribution_code: Bazi
    ui_registration: component
    block_registration: false
    runtime: native_react
    entry: default
    code_template_file: ui/components/Bazi.tsx
    code_template_version: 0.1.19
    code_template_language: tsx
```

This excerpt shows registration and the source entry. A complete declaration also requires code modules, context and permissions. Managed plugins continue to use `1flowbase.workspace.frontend-block-contribution` v1, `declarative_only` bindings and host-defined mounting grants.

## Package source references

The manifest declares registration metadata and file locations. Component implementations, editable templates and type declarations live in separate files. Host 0.6.3 supports:

| Field | File contents | Constraints |
| --- | --- | --- |
| `block_contributions[].code_template_file` | Component mounting entry or complete editable template | `.jsx` / `.tsx` matching `code_template_language`; up to 256 KiB |
| `code_modules[].type_declarations_file` | Complete TypeScript declarations | `.d.ts`; up to 2 MiB; also supported for settings-page modules |

These fields do not change `ui_registration` or `block_registration`. `code_modules[].source` remains the import specifier, not a file path. `assets[].path` continues to locate browser assets whose digests are explicitly declared.

Paths are relative to the plugin package root and the files must be included in the package. Absolute paths, `..`, backslashes and symlinks escaping the package root are rejected. During installation preparation, the host reads UTF-8 files and rejects missing files, directories, empty content, oversized content and conflicting source declarations. Existing inline `code_template` / `type_declarations` remain readable, but each item must choose either inline text or its corresponding file reference.

The installation transaction persists resolved source and declaration snapshots. It neither sends machine-local paths to the frontend nor rewrites the signed manifest. Editors and resource catalogs continue to consume host data. The calendar plugin ships these files under `ui/`, which the existing packager already includes. Its generator copies the composition and public types and generates host-binding declarations; YAML contains references rather than implementation text.

The host validates the manifest, persists the catalog and synchronizes component projections in the installation/version-switch transaction. Durable plugin ownership and contribution codes identify component records. Display names never identify ownership or overwrite manually managed content. Official templates are projected from the catalog rather than duplicated into the manual-template table.

`GET /api/console/frontend-blocks` returns currently authorized runtime contributions. Use `registration_target=block|component|code_template` to filter explicit targets. The backend supplies `registration_targets`. Block management and page creation request `block`; execution and imports retain the complete authorized catalog.

User copies live independently in `custom_frontend_blocks` and register only as `block`. Their names and source are independent snapshots; execution still requires the base contribution's current authority, version and assets. Deleting a definition does not delete source already saved in page instances.

The calendar plugin declares six components and one code template, all with `block_registration: false`. They must not be automatically exposed as directly creatable blocks.
