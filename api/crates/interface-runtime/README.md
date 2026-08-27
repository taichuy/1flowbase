# interface-runtime

`interface-runtime` owns the protocol-independent active interface contract for 1FlowBase. It compiles typed definitions and handler bindings into an immutable snapshot, publishes snapshots through `DynamicInterfaceRegistry`, and invokes a frozen snapshot through `InterfaceInvocationKernel`.

The request boundary is an authenticated `domain::ActorContext`. HTTP and MCP adapters remain responsible for credential parsing. The kernel resolves the interface, authorizes it, optionally calls the typed target-admission seam, invokes the typed handler, and records a terminal receipt carrying the registry and effective-graph fingerprints.

The crate intentionally does not own routers, OpenAPI generation, permission storage, extension discovery, Runtime Host state, database access, transport, or plugin lifecycle. The api-server composition root maps Effective Extension Graph declarations and application-owned handlers into the compiled registry. Consumers project dispatcher, catalog, permission, OpenAPI, and MCP discovery metadata from that active definition.

The target-admission port is the only I-02 evolution seam established here. It is not a hook executor or a multi-handler decision aggregator. Any expansion that requires an implementation-layer dependency or an untyped JSON/SQL/HTTP escape hatch must be reframed at the architecture Root.
