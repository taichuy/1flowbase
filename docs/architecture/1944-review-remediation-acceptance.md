# #1944 Review Remediation Acceptance Matrix

## Frozen input

- Reviewed product assembly: `15ed16b4a61b273d4a644fedeb1b53bb7de8c988`
- Reviewed documentation head: `b98f979947cbad40ec3461172d11c5df76938e2a`
- Review result: `REVIEW_FAIL` with three High and one Medium finding.
- Existing regression evidence remains valid only as compatibility evidence. It does not settle the
  architecture acceptance criteria below.

## Finite remediation matrix

| Row | Required observable result | Behavioral fixture |
| --- | --- | --- |
| RR-01 | An adapter selects an explicit `BindingId`; Kernel resolves that plan and derives the Interface identity from it | one Interface with HTTP and MCP bindings; MCP invocation receipt pins only the MCP binding |
| RR-02 | Envelope protocol and authentication source, Kernel authorization/admission ports and optional Hook Plan match the selected Compiled Plan | wrong protocol, AuthN, AuthZ, Admission and extension-plan negatives fail before Handler dispatch |
| RR-03 | Adapter plans belong to bindings and an ordered Extension Plan is compiled from registrations | one registry contains Public/User/Application bindings with distinct adapter identities; plan exposes internally computed ordered registrations/fingerprint |
| RR-04 | Effective handler selection and illegal extension registration fail during registry compilation/publish | missing/multiple Handler and illegal tier/point fixtures fail closed |
| RR-05 | Composition Root publishes one registry containing Public, Console, Application and MCP definitions/bindings | boot snapshot inventory proves the complete catalog; production routes only snapshot it and never compile request-time registries |
| RR-06 | Native async create, blocking execution and streaming execution have separate bindings/contracts | async completes after create; blocking completes after Runtime terminal; streaming uses a versioned server-stream event contract |
| RR-07 | Streaming Receipt remains Executing while events flow and reaches exactly one terminal before Projected | live stream fixture observes event → terminal receipt → adapter projection order and a dispatch-time Runtime target pin |
| RR-08 | Existing external API, authorization, DTO, SSE order, Runtime behavior, migrations and official plugin contracts remain unchanged | full centralized Test Batch plus existing route/runtime/plugin suites |

## Work packets

1. `RR-F01`: binding-first envelope/resolve, per-binding adapter plan and controlled negatives.
2. `RR-F02`: Registry-owned ordered Extension Plan and effective Handler compilation.
3. `RR-F03`: boot-time unified Dynamic Interface Registry and removal of request-time compilers.
4. `RR-F04`: Native async/blocking/server-stream lifecycle with terminal-before-projection.
5. `RR-F05`: replace source-string/equivalence self-report fixtures with behavioral assembly and
   lifecycle fixtures; update Receipt and Ledger truth.

All fixtures are assembled before execution. No per-packet test, per-layer QA or partial rerun may
settle these rows. After the remediation assembly is frozen, the complete #1944 centralized Test
Batch runs fresh. Any Blocking/High fix invalidates that run.

## Stop conditions

- External API, permission, migration, user data, Runtime or official plugin behavior must change.
- Binding selection needs a fallback or first-plan lookup.
- Streaming completion must be declared before its Runtime/event terminal.
- Registry compilation needs Axum, control-plane, Storage, Plugin Framework or Runtime Host inside
  `interface-runtime`.
- Extension registration cannot be reduced to an ordered typed plan at Composition Root.
