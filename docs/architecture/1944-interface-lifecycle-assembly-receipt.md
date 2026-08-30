# #1944 Interface Lifecycle Assembly and QA Receipt

## Result and identities

- Result: `QA_PENDING` for the XR executable-contribution assembly
- Input: `beta@ff4cc74ab073256419884d3d96e0b3defcb36d45`
- Previous fresh tested assembly: `beta@a3c78798320b5e2af8bc8b2f9b35cb8fe3977b31`
- XR input: `beta@965d62e9514f1b3e25fdf2a4284cc3bb41cfbf2e`
- XR assembly: pending XR-F05 freeze
- Official plugins: `main@8bf11605b02a0df8dd01271875f1ec3d182c0d3a`
- Fresh centralized QA: executable-extension remediation attempt 11, 16/16 rows complete, zero unrun rows
- Evidence root: `tmp/test-governance/1944-interface-lifecycle/`

The earlier QA evidence remains valid only for the previous product assembly. It does not settle
RR-12 through RR-16; the XR assembly requires one fresh centralized QA after XR-F05 is frozen.

## XR executable-contribution packet ledger

| Packet | Commit | Status | Principal write set |
| --- | --- | --- | --- |
| XR-F01 | `3817d8f0d` | ASSEMBLED | RR-12–RR-16 matrix and controlled fixtures |
| XR-F02 | `b12754554` | ASSEMBLED | Hook input/output contract identity and publish-time validation |
| XR-F03/XR-F04 | `d26433d03` | ASSEMBLED | Definition/AuthN activation and ordered AuthZ/Admission executable plans |
| XR-F05 | pending | ACTIVE | production composition, boundaries, ADR, rules and receipt |
| XR-F06 | pending | READY | frozen assembly and fresh centralized QA |

No ARC acceptance criterion is settled by this pre-QA ledger.

## Work Packet ledger

| Packet | Commit | Status | Principal write set |
| --- | --- | --- | --- |
| IF-F01 | `614b15683` | PASS | ADR, finite inventory, machine-readable fixture, frozen acceptance matrix and Test Batch |
| IF-F02 | `a216dd0f3` | PASS | `interface-runtime` Definition, Binding, Compiled Plan and registry compiler |
| IF-F03 | `f8c57ff12` | PASS | sealed Public/User/Application principals, typed envelope, summaries and adapter projections |
| IF-F04 | `3a3285d4a` | PASS | lifecycle stages, attempts, pins, result/stream contracts and receipts |
| IF-F05 | `2ad051983` | PASS | Public, Console, Application/SSE and MCP production vertical slices |
| IF-F06 | `2538872a4` | PASS | typed extension points, tier capability matrix and compiler negatives |
| IF-F07 | `c4c0ba731` | PASS | route-equivalence fixtures, gap ledger and controlled-negative boundaries |
| IF-F08 | receipt commit | PASS | this Assembly/QA Receipt and GitHub Ledger receipts |

QA fix packets, all inside the approved boundary:

| Commit | Reason |
| --- | --- |
| `e21db1391` | stable handler future alias syntax |
| `b77431d6b` | typed Public adapter error conversion |
| `9e27b6234` | preserve MCP typed server delegation and Console projected receipt |
| `dca6b0e01` | align the pre-typed Node fixture to `InvocationEnvelope::with_principal` |
| `15ed16b4a` | align the same fixture to typed Console `UserPrincipal` transfer |
| `cb0597381` | freeze the finite review-remediation acceptance matrix |
| `7b7354d38` | make Binding identity drive resolve; compile per-binding adapters and ordered extensions |
| `585a2e579` | move Native Runtime and server-stream terminal inside the Invocation lifecycle |
| `1bd0be886` | publish the unified Dynamic Interface Registry at Composition Root |
| `ed87ad79e` | replace source/self-report probes with behavioral assembly fixtures |
| `281e6c58f` | bind the typed Hook Plan to the compiled Extension Plan fingerprint |
| `204a79a2a` | align strict Admission and Route fixtures without weakening validation |
| `42374780a` | preserve the typed MCP context module boundary |
| `2f132b4db` | rebuild the catalog after test-state customization |
| `c3ed93378` | publish the complete catalog for every router composition and authorize non-HTTP Providers projections through the canonical Console operation |
| `d3b28fce9` | include the approved typed stream module in the controlled facade inventory |
| `798b25104` | freeze executable-extension acceptance rows RR-09 through RR-11 |
| `dd9874bc2` | bind concrete typed hooks and contributed handlers into Compiled Invocation Plans; make unary and stream consumption mandatory |
| `c23178a9e` | remove fixture constructor shadowing found by centralized QA attempt 8 |
| `bafe9e2e9` | stabilize the centralized cache TTL fixture and remove the two #1944 api-server dead-code warnings |
| `a3c787983` | make the Node boundary require snapshot-owned hooks and reject Route-level Hook Plan injection |

## Final contract structure

- Canonical `InterfaceDefinition` owns business invocation identity, versioned contracts,
  access/authentication/authorization/admission policies, owner and execution semantics.
- `ProtocolBinding` independently owns the HTTP/MCP/protocol projection and binding identity;
  adapters provide an explicit `BindingId`, and the Kernel rejects protocol/binding mismatch.
- `CompiledInvocationPlan` freezes definition, binding, per-binding adapter plan, the concrete
  effective typed Handler, concrete typed lifecycle Hook bindings and the actual ordered Extension
  Plan plus graph/plan fingerprints. Registry publication rejects missing, extra, misordered or
  graph-mismatched executable bindings as well as duplicate, unknown, inactive and illegal input.
- `InvocationEnvelope<Input, Principal>` is typed over sealed `PublicPrincipal`, `UserPrincipal`
  and `ApplicationPrincipal`. Only trimmed `PrincipalSummary` reaches hooks and receipts.
- Resolve-time pins definition/binding/graph/registry/hook plan. Dispatch-time pins attempt,
  handler/plugin/artifact/runtime/worker generation. Retry creates a new attempt.
- The lifecycle is Received → Resolved → PrincipalEstablished → Authorized → Admitted →
  Prepared → Dispatched → Executing → PostProcessed → terminal → Projected. Unary,
  server-stream and async-ack results are typed; every invocation has exactly one terminal.
  Interface completion is not a persisted Domain Event.

## Four production vertical slices

| Slice | Result | Preserved boundary |
| --- | --- | --- |
| Public login instances | PASS | Public principal, locale/order/default authenticator DTO and error behavior |
| Console providers | PASS | User principal, Console operation permission, row scope and existing DTO/error mapping |
| Application native run + SSE | PASS | Application/API-key/workspace/Actor identity, runtime dispatch and SSE event ordering |
| MCP User API key | PASS | User API-key principal, JSON-RPC result/error/continuation behavior and server-delegated internal authorization without raw credentials |

Compatibility APIs, `/api/ex`, WebSocket variants, sign-in mutation and background workers
remain regression-only entries. They were not silently migrated and have no fallback or double
write.

## Interface Extension Space

| Tier | Definition | Authentication | AuthZ/Admission/hooks | Handler | Isolation |
| --- | --- | --- | --- | --- | --- |
| Built-in | allowed | allowed | typed facts; explicit mutation permission | allowed, exactly one | trusted in-process |
| HostExtension | allowed | allowed | typed facts; explicit mutation permission | allowed, exactly one | trusted in-process |
| RuntimeExtension | permission-scoped | forbidden | permission/scoped typed facts | permission-scoped | process/wire |
| CapabilityPlugin | permission-scoped | forbidden | permission/scoped typed facts | permission-scoped | process/wire |

The approved points are `interface.definition`, `interface.authentication_adapter`,
`interface.authorization`, `interface.admission`, `interface.before`, `interface.handler`,
`interface.after`, `interface.failure` and `interface.completion`.

Before/After/Failure/Completion registrations are paired exactly with typed executable bindings in
registration order. Unary and server-stream Kernels obtain that plan only from the frozen registry;
Routes cannot inject `None` or replace it. A HostExtension Handler registration must bind a real
typed implementation and becomes the sole effective Handler; zero or multiple effective bindings
fail registry publication.

## ARC acceptance evidence

| AC | Status | Evidence |
| --- | --- | --- |
| ARC-AC-001 | PASS | 10-entry/9-family finite inventory and full API regression |
| ARC-AC-002 | PASS | unified boot Catalog plus separate Definition/Binding/per-binding Compiled Plan tests |
| ARC-AC-003 | PASS | Native async/blocking/server-stream bindings and versioned input/output/stream/error/terminal contracts |
| ARC-AC-004 | PASS | sealed principal unit and compile-fail tests |
| ARC-AC-005 | PASS | credential-boundary source fixture and four real adapters |
| ARC-AC-006 | PASS | unary and server-stream automatically execute the compiled Before/After/Failure/Completion plan; Native Runtime executes before terminal |
| ARC-AC-007 | PASS | live stream event → terminal → projection plus retry/deadline/cancel negatives |
| ARC-AC-008 | PASS | Native dispatch freezes attempt and Runtime/worker generation; replacement negative |
| ARC-AC-009 | PASS | Binding-first resolve, HTTP/MCP dual-binding and mismatch controlled negatives |
| ARC-AC-010 | PASS | typed Handler ports and Node infrastructure-import negatives |
| ARC-AC-011 | PASS | four-tier/nine-point capability matrix plus concrete ordered Hook bindings and a real exactly-one contributed Handler |
| ARC-AC-012 | PASS | completion/receipt versus outbox separation tests |
| ARC-AC-013 | PASS | extension illegal-point and duplicate/missing/mismatch/inactive publish negatives |
| ARC-AC-014 | PASS | behavioral boot/vertical fixtures plus full four-path API regression |
| ARC-AC-015 | PASS | Cargo dependency and Node source-boundary controlled negatives |
| ARC-AC-016 | PASS | fresh executable-extension remediation attempt 11, 16/16 rows, zero unrun |

## Controlled negatives

All approved controlled negatives passed: duplicate Interface/Binding identity; unknown AuthZ
operation; missing/mismatched typed Handler; binding contract/version mismatch; inactive owner;
illegal extension point; Public ActorContext injection; missing/mismatched Application identity;
Handler infrastructure imports; Runtime/Capability authentication registration;
`interface-runtime` forbidden dependencies; multiple stream terminals; attempt reuse; dispatched
runtime-generation replacement; and treating Interface completion as a persisted Domain Event.

## Fresh centralized QA receipt

| Row | Result | Count / command class |
| --- | --- | --- |
| QA-001 | PASS | paired source identity and cleanliness |
| QA-002 | PASS | `cargo fmt --all --check` |
| QA-003 | PASS | interface-runtime 25 unit + 2 compile-fail doc tests |
| QA-004 | PASS | access-control 35 tests |
| QA-005 | PASS | api-server 1199 tests, zero ignored |
| QA-006 | PASS | Node boundary 5 tests |
| QA-007 | PASS | runtime-core 32, orchestration-runtime 388, runtime-extension-host 59 tests; its 2 ignored real-Host tests executed in QA-010 |
| QA-008 | PASS | official plugin Node 153 tests |
| QA-009 | PASS | 9 official executable `cargo build --locked` commands |
| QA-010 | PASS | 2 ignored real-Host conformance tests explicitly executed |
| QA-011 | PASS | migration tree equals input; zero migration diff |
| QA-012 | PASS | dev-up/deploy/rollback 79 tests and 4 compose configs |
| QA-013 | PASS | `cargo check --workspace` |
| QA-014 | PASS | `cargo metadata --locked --offline` |
| QA-015 | PASS | diff check and paired final integrity |
| QA-016 | PASS | 16/16 rows; unrun 0 |

Fresh automated tests total `1979 passed`, `0 failed`. The only non-blocking warnings are the
existing 19 private dead-code warnings in `runtime-extension-host`; the #1944 boot import and
unused Native stream helper warnings were removed before the final frozen assembly.

## Compatibility and repository integrity

- Database schema, migration tree and user data: unchanged.
- External API/DTO/status/error, authorization and row-scope results: preserved by full API and
  route-equivalence tests.
- Runtime behavior, stream order, stdio wire and plugin manifests: preserved by runtime suites,
  official Node suites, 9 executable builds and real Host conformance.
- QA ended at the tested product assembly with only the two protected private memory changes;
  documentation-only Receipt commits follow that assembly.
- Official plugin repository ended clean at `8bf11605b02a0df8dd01271875f1ec3d182c0d3a`.
- No push was performed. #1944 and #1893 remain open. Root acceptance criteria were not settled.
