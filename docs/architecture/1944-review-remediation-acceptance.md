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
| RR-09 | Every lifecycle registration has a concrete typed executable binding owned by the compiled plan; missing bindings fail publish | Completion registration without binding fails; ordinary `invoke` executes a bound Completion hook without Route injection |
| RR-10 | Unary and server-stream consume the same frozen Before/After/Failure/Completion rules | stream fixture observes Before → events → terminal → After → Completion before projection |
| RR-11 | A Handler contribution binds a real typed implementation and becomes the exactly-one effective target | one HostExtension handler executes; missing binding or multiple effective contributions fail compilation |
| RR-12 | An erased Hook Plan exposes stable typed input/output contract identities and mismatches fail during `RegistryCompiler::compile()` | wrong-input and wrong-output Hook Plans fail publication for unary and server-stream Definitions |
| RR-13 | A Definition registration is executable compilation input that contributes a typed Definition and required Protocol Bindings into the canonical Registry | metadata-only, duplicate identity/version, inactive owner, unknown operation and binding/contract conflict fixtures fail publication |
| RR-14 | BuiltIn/HostExtension Authentication contributions bind a real credential-consuming factory owned by the Composition Root; the Protocol Adapter resolves the frozen activation, invokes that factory, then establishes the sealed Principal | Public/Console/Application/MCP factories execute existing authentication owners; trusted HostExtension success/reject and credential-contract fixtures execute; missing, extra, duplicate, inactive and identity-mismatched factories fail before router/catalog publication; forbidden Runtime/Capability tiers remain rejected |
| RR-15 | The Compiled Plan owns ordered executable Authorization veto contributions after the mandatory core decision | unary and server-stream prove core-deny dominance, ordered allow, and extension deny/error/deadline fail closed; binding/permission/Graph/contract mismatches fail publication |
| RR-16 | The Compiled Plan owns ordered executable Admission veto contributions after mandatory target admission | unary and server-stream prove Authorization → core Admission → ordered extension Admission → Hook → Handler; missing/extra/order/facts/contract mismatches and reject/error/deadline fail closed |

## Work packets

1. `RR-F01`: binding-first envelope/resolve, per-binding adapter plan and controlled negatives.
2. `RR-F02`: Registry-owned ordered Extension Plan and effective Handler compilation.
3. `RR-F03`: boot-time unified Dynamic Interface Registry and removal of request-time compilers.
4. `RR-F04`: Native async/blocking/server-stream lifecycle with terminal-before-projection.
5. `RR-F05`: replace source-string/equivalence self-report fixtures with behavioral assembly and
   lifecycle fixtures; update Receipt and Ledger truth.
6. `RR-F06`: compile concrete typed extension bindings into every plan, make Kernel consumption
   non-optional for unary/stream, and compile a real exactly-one contributed Handler.
7. `XR-F01`: freeze RR-12 through RR-16 and their controlled fixture inventory.
8. `XR-F02`: publish-time Hook input/output contract identity.
9. `XR-F03`: executable Definition contribution and activated Authentication binding.
10. `XR-F04`: ordered Authorization/Admission executable veto plans.
11. `XR-F05`: four production vertical paths, source boundaries and architecture receipts.
12. `XR-F06`: freeze the replacement assembly and run the complete fresh centralized Test Batch.
13. `XR-A01`: freeze the RR-14 credential-to-Principal factory remediation matrix.
14. `XR-A02`: bind real BuiltIn/HostExtension Authentication factories at Composition Root and
    migrate the four production adapters to factory-first authentication.
15. `XR-A03`: freeze the replacement assembly and run the complete fresh centralized Test Batch.

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
