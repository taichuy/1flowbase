# #1944 Interface Lifecycle Assembly and QA Receipt

## Result and identities

- Result: `QA_PENDING`
- Input: `beta@ff4cc74ab073256419884d3d96e0b3defcb36d45`
- Previous fresh tested assembly: `beta@a3c78798320b5e2af8bc8b2f9b35cb8fe3977b31`
- XR input: `beta@965d62e9514f1b3e25fdf2a4284cc3bb41cfbf2e`
- Fresh tested XR product assembly: `beta@4b31cc86c2d7e74e053ac5c0b7976031265d7091`
- Authentication closure input: `beta@efa5df40dfc11e9b703d5ecfa1446f3a9dfb28d3`
- Current untested product assembly: `beta@b85b9a705c98acece4e5f138c345a911a9d704cb`
- Official plugins: `main@8bf11605b02a0df8dd01271875f1ec3d182c0d3a`
- Previous centralized QA: executable-contribution XR attempt 16, 16/16 rows complete, zero unrun rows; superseded as architecture acceptance evidence by the reopened RR-14 finding
- Evidence root: `tmp/test-governance/1944-interface-lifecycle/`

XR attempt 16 remains valid historical compatibility evidence for product assembly `4b31cc86c`,
but it cannot settle RR-14, ARC-AC-006, ARC-AC-011 or the Authentication portion of ARC-AC-013.
Authentication attempt 21 is `QA_FAIL`: it started at the real candidate
`1f6e68335b59837c93a655ce810eaa59a89af6b4`, ended after unrelated branch drift, left QA-016
unrun, and its manually typed receipt SHA was invalid. XR-A04/XR-A05 replace that candidate. A new
QA receipt may be written only after XR-A06 captures the complete assembly with `git rev-parse
HEAD` and QA-001 through QA-016 run against that unchanged identity.

XR-A06 attempt 22 froze `c740e28d9135d9dd0958bb619a601b485a8848e2` and passed QA-001 through
QA-004, then QA-005 found a Rust ownership compile error in the new Providers adapter: the Route
borrowed the resolved activation from the snapshot and later moved the snapshot into the Kernel.
`b85b9a705c98acece4e5f138c345a911a9d704cb` owns a clone of that already-frozen typed activation;
attempt 22 is invalidated in full and the replacement QA must restart at QA-001.

## XR executable-contribution packet ledger

| Packet | Commit | Status | Principal write set |
| --- | --- | --- | --- |
| XR-F01 | `3817d8f0d` | ASSEMBLED | RR-12–RR-16 matrix and controlled fixtures |
| XR-F02 | `b12754554` | ASSEMBLED | Hook input/output contract identity and publish-time validation |
| XR-F03/XR-F04 | `d26433d03` | ASSEMBLED | Definition/AuthN activation and ordered AuthZ/Admission executable plans |
| XR-F05 | `461e981fe` | ASSEMBLED | production composition, boundaries, ADR, rules and receipt |
| XR-F06 | `4b31cc86c` | PASS | frozen replacement assembly and fresh centralized QA |
| XR-A01 | `c1bbe59fe` | ASSEMBLED | RR-14 real credential-to-Principal factory matrix and controlled fixtures |
| XR-A02 | `e7df1aae4` | ASSEMBLED | Composition Root credential-consuming factory Port and four production adapters |
| XR-A03 | `1f6e68335` | QA_FAIL | attempt 21: 14 PASS / 1 FAIL / 1 UNRUN because frozen identity drifted |
| XR-A04 | `258705de1` | ASSEMBLED | Console single-owner AuthN and installable HostExtension AuthN closure fixtures |
| XR-A05 | `2d97d3559` | ASSEMBLED | middleware/Route ownership and manifest→Graph→Registry→factory→Route assembly |
| XR-A05-F1 | `b85b9a705` | ASSEMBLED | own the resolved frozen activation before moving the registry snapshot into Kernel |
| XR-A06 | pending | ACTIVE | replacement frozen assembly and QA-001–QA-016 restart with command-captured SHA |

QA fix commits inside the approved XR boundary: `f35b3b261` removed fixture shadowing and warnings;
`d14311934` aligned ordered registration evidence; `65470db64` completed typed route/error assembly;
`4b31cc86c` aligned the exact public-facade inventory. ARC statuses below are candidates for user
acceptance; Root AC is not formally settled by this Delivery receipt.

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
  activated Authentication identity, core adapter references, ordered typed Authorization and
  Admission veto bindings, the concrete effective typed Handler, typed lifecycle Hook bindings and
  the actual ordered Extension Plan plus graph/plan fingerprints. Registry publication rejects
  missing, extra, misordered, permission/contract or graph-mismatched executable bindings as well
  as duplicate, unknown, inactive and illegal input.
- Typed Definition contributions compile into the same canonical registry and contribute their
  required Protocol Bindings before publication. `api-server` Composition Root owns trusted
  Authentication factories; Protocol Adapters resolve only the factory frozen by the binding plan
  before constructing a sealed Principal. Core business Authorization/Admission remain mandatory;
  ordered extension decisions are additive fail-closed vetoes and cannot replace a core denial.
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
| ARC-AC-006 | PENDING | lifecycle execution is assembled, but Authentication must be proven inside the frozen adapter-to-Principal chain on the replacement assembly |
| ARC-AC-007 | PASS | live stream event → terminal → projection plus retry/deadline/cancel negatives |
| ARC-AC-008 | PASS | Native dispatch freezes attempt and Runtime/worker generation; replacement negative |
| ARC-AC-009 | PASS | Binding-first resolve, HTTP/MCP dual-binding and mismatch controlled negatives |
| ARC-AC-010 | PASS | typed Handler ports and Node infrastructure-import negatives |
| ARC-AC-011 | PENDING | Definition/AuthZ/Admission/Hook/Handler bindings are assembled; real BuiltIn/HostExtension Authentication factory execution awaits replacement QA |
| ARC-AC-012 | PASS | completion/receipt versus outbox separation tests |
| ARC-AC-013 | PENDING | existing compiler negatives pass; missing/extra Authentication factory publication fixtures await replacement QA |
| ARC-AC-014 | PASS | behavioral boot/vertical fixtures plus full four-path API regression |
| ARC-AC-015 | PASS | Cargo dependency and Node source-boundary controlled negatives |
| ARC-AC-016 | PENDING | latest Authentication attempt 21 failed frozen-identity integrity and left QA-016 unrun; XR-A06 must restart all rows |

## Controlled negatives

All approved controlled negatives passed: duplicate Interface/Binding identity; unknown AuthZ
operation; missing/mismatched typed Handler; binding contract/version mismatch; inactive owner;
illegal extension point; Public ActorContext injection; missing/mismatched Application identity;
Handler infrastructure imports; Runtime/Capability authentication registration;
`interface-runtime` forbidden dependencies; multiple stream terminals; attempt reuse; dispatched
runtime-generation replacement; and treating Interface completion as a persisted Domain Event.

## XR acceptance evidence

| Requirement | Status | Direct evidence |
| --- | --- | --- |
| RR-12 | PASS | unary and stream wrong input/output Hook contracts fail in `RegistryCompiler::compile()` |
| RR-13 | PASS | typed Definition contribution materializes Definition/Binding; metadata-only, duplicate and inactive contributions fail publication |
| RR-14 | PENDING | previous factories only validated already-built Principals; replacement must prove frozen-plan factory selection, real credential authentication, trusted HostExtension success/reject, and bidirectional missing/extra/mismatch publication failure |
| RR-15 | PASS | unary/stream execute core then ordered Authorization vetoes; core deny dominates and extension deny/error/deadline fail closed |
| RR-16 | PASS | unary/stream execute core then ordered Admission vetoes; missing/extra/order/Graph/contract and reject/error/deadline cases fail closed |

## Fresh centralized QA receipt

| Row | Result | Count / command class |
| --- | --- | --- |
| QA-001–QA-014 | SUPERSEDED | attempt 21 recorded 14 passing rows and 1992 automated tests, but branch drift prevents binding them to one frozen candidate |
| QA-015 | FAIL | start `1f6e68335b59837c93a655ce810eaa59a89af6b4`; end `2b08d777940be1e60b3ee8ac96a88c6b4d7bf60b` plus unrelated dirty files |
| QA-016 | UNRUN | attempt 21 stopped after final-integrity failure |
| attempt 22 QA-001–QA-004 | INVALIDATED PASS | identity/fmt/interface-runtime/access-control passed on `c740e28d9`; superseded by the compile fix |
| attempt 22 QA-005 | FAIL | `api-server` compile rejected a borrowed activation surviving the snapshot move |
| attempt 22 QA-006–QA-016 | UNRUN | batch stopped at the first product Blocking |
| XR-A06 QA-001–QA-016 | PENDING | must run from the beginning against one command-captured unchanged SHA |

The superseded XR compatibility run recorded `1987 passed`, `0 failed`; Authentication attempt 21
recorded `1992 passed`, `0 failed` before its integrity failure. No test has run against
`b85b9a705c98acece4e5f138c345a911a9d704cb`. The next valid result must come from one complete
fresh centralized run after XR-A06 freezes the replacement assembly.

## Compatibility and repository integrity

- Database schema, migration tree and user data: unchanged.
- External API/DTO/status/error, authorization and row-scope results: preserved by full API and
  route-equivalence tests.
- Runtime behavior, stream order, stdio wire and plugin manifests: preserved by runtime suites,
  official Node suites, 9 executable builds and real Host conformance.
- Latest QA attempt did not end at its frozen candidate and is explicitly `QA_FAIL`; no compatibility
  conclusion is promoted from that attempt. XR-A06 must re-establish paired identity and cleanliness.
- Official plugin repository ended clean at `8bf11605b02a0df8dd01271875f1ec3d182c0d3a`.
- No push was performed. #1944 and #1893 remain open. Root acceptance criteria were not settled.
