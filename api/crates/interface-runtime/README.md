# interface-runtime

`interface-runtime` owns the protocol-independent active interface contract for 1FlowBase. Canonical `InterfaceDefinition`, protocol `ProtocolBinding`, and immutable `CompiledInvocationPlan` are separate objects: business semantics, transport projection, and frozen execution wiring cannot overwrite one another. The compiler publishes their deterministic fingerprints through `DynamicInterfaceRegistry`, and `InterfaceInvocationKernel` invokes a frozen plan.

The request boundary is `InvocationEnvelope<Input, Principal>` with sealed `PublicPrincipal`, `UserPrincipal`, and `ApplicationPrincipal` profiles. HTTP and MCP adapters terminate raw credential propagation and project a typed principal; `ActorContext` remains the authorization truth inside User/Application profiles. Hooks, admission, and receipts receive only `PrincipalSummary`. The kernel resolves the interface, authorizes it, optionally calls typed target admission, invokes the profile-specific typed handler, and records a terminal receipt carrying frozen identities.

The crate intentionally does not own routers, OpenAPI generation, permission storage, extension discovery, Runtime Host state, database access, transport, or plugin lifecycle. The api-server composition root maps Effective Extension Graph declarations and application-owned handlers into the compiled registry. Consumers project dispatcher, catalog, permission, OpenAPI, and MCP discovery metadata from that active definition.

The target-admission port is the only I-02 evolution seam established here. It is not a hook executor or a multi-handler decision aggregator. Any expansion that requires an implementation-layer dependency or an untyped JSON/SQL/HTTP escape hatch must be reframed at the architecture Root.
