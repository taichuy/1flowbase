# #1944 Four Production Vertical Slices

All four approved slices preserve their existing protocol adapters and business services. The migration inserts the typed interface lifecycle after authentication/protocol parsing and before the existing application target. Receipt projection remains adapter-owned and is not a domain event.

| Slice | Adapter → Principal | Typed target | Projection and equivalence boundary |
| --- | --- | --- | --- |
| Public login instances | HTTP headers resolve `CatalogLocale`; no credential; `PublicPrincipal` | Narrow public-login query port | Existing `ApiSuccess<PublicLoginInstancesResponse>`, localization, ordering and default authenticator logic |
| Console providers | Existing session/API-key authentication; `UserPrincipal` | Existing host-infrastructure query port | Existing DTO, permission operation and HTTP error mapping |
| Application native run + SSE | Bearer token authenticates once to `ApplicationApiKeyActor`; `ApplicationPrincipal` retains application/API-key/workspace/Actor | Authenticated-actor native-run port; control-plane no longer needs the token on this path | Existing 201 response modes, blocking execution, SSE event sender/order and runtime dispatch; runtime MCP delegation is actor-based |
| MCP User API Key | Existing `require_session`; only User API Key accepted; `UserPrincipal::UserApiKey` | Typed MCP method enum and bounded tool-arguments wrapper over the existing Virtual UI dispatch port | Existing JSON-RPC ids, status codes, error codes/data, response-size cap, tool list and call result projection |

The compatibility APIs, `/api/ex`, WebSocket routes, sign-in mutation and background workers remain regression-only gap-ledger entries. They were not placed on a parallel production path and received no fallback or double write.
