# Plugin frontend UI registration

[中文](frontend-ui-registration.md)

`block_contributions[].ui_registration` explicitly selects the management destination:

| Value | Destination | Behavior |
| --- | --- | --- |
| `code_template` (default when omitted) | Settings → UI Management → Code Templates | A complete block template for copying, editing and publishing. Does not automatically create a UI component record. |
| `component` | Settings → UI Management → Components | An automatically registered, plugin-owned read-only UI component. Excluded from both settings and workspace code-template lists. |

The destinations are mutually exclusive. The presence of `code_template` source alone must not register a contribution in both places.
`component` requires `native_react`, nonempty `code_template`, a version and JSX/TSX language.
The source still supports block mounting and component insertion; the destination does not change the runtime module/asset loading contract.

```yaml
block_contributions:
  - contribution_code: Calendar
    title: Calendar
    runtime: native_react
    entry: default
    ui_registration: component
    code_template: "import { Calendar } from '@example/calendar'; export default () => <Calendar />;"
    code_template_version: 1.0.0
    code_template_language: tsx
    # Declare code_modules, context_contract and permissions as required by the manifest contract.
```

Plugins declare contributions only. The host owns transactional installation, idempotent publisher/contribution upgrades, read-only provenance and uninstall cleanup.
Existing manually maintained templates and components are not deleted or rewritten. Older packages without this field remain code templates; selecting Components requires a plugin version upgrade with an explicit `component` declaration.
A host source build supporting this field is required; an older published binary with the same version number is not necessarily compatible.
