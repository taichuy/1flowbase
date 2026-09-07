# Authentication entry disposition — Root #1998

Source audit at P5 assembly (after P4 `1977dac9224699fd6de152649fc3a65a28137b28`). This is a finite implementation inventory, not an execution plan.

The inventory is 16 original direct route factory sites plus P2 WebMCP. Final source has 16 common-owner calls: the old compatibility `invoke_stream` owner was retired after its callers converged on the authenticated stream path. Every credential-authenticated resolved invocation uses `ExtensionBootSnapshot::authenticate_invocation`, the frozen factory, and `AuthenticatedInvocation::into_envelope`. The attempt ID is allocated before authentication. Rejection awaits frozen Completion with the shared 1000 ms finalization budget and publishes safe `interface_lifecycle` tracing metadata; original errors stay in the host for existing projections.

| Original site | Disposition and projection |
| --- | --- |
| `console_interface.rs::invoke` | P4: common owner; original ApiError; deferred CSRF remains admission. |
| `console_interface.rs::invoke_server_stream` | P4: same owner/lineage and admission; explicit stream completion owner retained. |
| `mcp_protocol.rs::handle_mcp_request` | Common owner with MCP protocol; failed credentials retain `NotAuthenticated` HTTP response; JSON-RPC validation/dispatch remains separate. |
| `identity/auth.rs::list_login_entries` | Public activation attempt; same lineage enters public business handler. |
| `identity/auth.rs::sign_in` | Public activation attempt. Password login rejection belongs to the sign-in handler, not authentication-adapter rejection. |
| `identity/auth.rs::invoke_public_residual` | Public activation attempt for residual public bindings, same lineage. |
| `identity/console_identity_interface.rs::invoke` | Common owner, original ApiError, no change to credential/CSRF selection. |
| `application_public_api/compatibility_interface.rs::authenticate_application_principal` | Returns an authenticated invocation carrying frozen snapshot and attempt, or later explicitly yields a principal for WS handshake. Original Native NotAuthenticated projection. |
| `application_public_api/compatibility_interface.rs::invoke_blocking` | Common owner; same attempt/envelope and application dispatch target. |
| `application_public_api/compatibility_interface.rs::invoke_stream` | Retired as unused after HTTP callers authenticate once and use `invoke_stream_with_principal`; common-owner authentication and the existing shared stream supervisor now cover this former site. |
| `application_public_api/compatibility_interface/models.rs::invoke` | Common owner; original native error conversion. |
| `application_public_api/native.rs::create_native_run` | Common owner for blocking/stream/async bindings; original native error conversion. |
| `application_public_api/native.rs::authenticate_native_binding` | Returns snapshot plus authenticated invocation; read/upload helpers consume its envelope without allocating another root. |
| `application_public_api/ex.rs::invoke_workflow_extension` | Common owner; ProtocolWithCsrf and workflow-specific error conversion retained. |
| `plugins_and_models/runtime_models/interface.rs::invoke` | Common owner; original runtime ApiError and route-dependent CSRF policy retained. |
| `settings/host_infrastructure/interface_operation.rs::invoke_providers_view` | Credentials and the existing ServerDelegation adapter flow use the common owner and retain the selected factory, then enter the same kernel. |
| P2 `webmcp/protocol.rs::invoke` | Common owner; outer Cookie-only/CSRF/API-key/exposure constraints and response projections retained. |

## Same-request and established-principal consumers

`ApplicationInvocationAuthentication` represents the actual two cases: HTTP authentication retains its original frozen snapshot/attempt; pre-established WebSocket principals create a fresh invocation per turn. `invoke_*_with_principal` accepts either case and continues through the same authorization/kernel path.

Five original authentication helper consumer sites are in four files: Anthropic HTTP; OpenAI Chat HTTP; OpenAI Responses HTTP; Native WS upgrade; Responses WS upgrade. HTTP resume paths that fall back to a new turn retain their authenticated attempt and do not authenticate again. WS upgrades explicitly extract the principal; later WS commands do not reuse the handshake ID or reauthenticate credentials.

`extension_bus/interface_contributions.rs::ApiMcpDebugActivatedOperations::providers_view` receives an established `&UserPrincipal`, clones its actor into `ServerDelegation`, and calls providers-view. This existing delegation adapter continues through the selected replaceable factory and common owner: current built-in/acme factories only wrap the established actor without looking up credentials, but custom factory authority is preserved. This is not changed into a factory bypass. `Console invoke_with_principal`, native/Responses WS turn bridges and runtime MCP actor delegation retain their existing established-principal paths. Native/compat stream `complete().await` ownership is unchanged; dropped futures do not claim observer execution.

## Ingress and bootstrap boundaries

Missing bearer headers, malformed JSON/protocol frames, WS protocol negotiation, request parsers and authentication bootstrap remain outside the resolved authentication-attempt guarantee. Static route activation guards keep their existing unavailable responses. Unknown binding or unavailable factory reaching the common owner emits finite ingress diagnostics (`unresolved-authentication-binding` / `unresolved-authentication-activation`) without a fabricated business receipt. Actual adapter rejection classes are only `credential-rejected` and `authentication-failed`; neither credentials nor original error text enters receipts or plugin context.

The legacy boot `authenticate` wrapper has no remaining consumers and is removed. The real factory authentication implementation remains host-owned. Principal access is used by native/MCP/compat adapters; the lineage accessor is test-only for continuity evidence.

## Evidence deferred to the Root Test Batch

- `root_1998_authentication_rejection_uses_frozen_plan_and_absent_principal`: actual frozen Completion, absent identity and receipt pins.
- `root_1998_console_unary_and_stream_rejection_preserve_http_and_publish_safe_record`: Console errors and actual safe log publication.
- `root_1998_success_reuses_attempt_lineage_and_unknown_binding_is_ingress_only`: success continuity and missing binding/factory boundaries.
- `root_1998_mcp_identity_native_webmcp_http_rejections_publish_correlated_safe_terminal`: real router responses and correlated safe logs across four migrated protocols/families.
- Source authenticity mutation rejects direct factory bypass and discarded attempt lineage; existing no-route-authentication/no-hook-plan-injection gates remain.
- Existing WebMCP Cookie/CSRF/exposure, native response, compatibility resume/WS and provider-delegation fixtures remain the regression evidence. No P5 behavior tests were run before Root freeze.
