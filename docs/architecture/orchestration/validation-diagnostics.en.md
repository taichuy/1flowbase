# Flow configuration diagnostics

[中文](validation-diagnostics.md)

`orchestration-runtime` owns node configuration, binding, and topology validation. Recoverable compilation findings remain `CompileIssue` records, with an optional `field_path`. Configuration failures that prevent plan construction carry typed `FlowValidationError` / `NodeDiagnostic` values. This classification applies at the pure compiler boundary; database, network, and provider execution failures are not reclassified as invalid configuration.

The service preserves diagnostics. A node preview still checks collected issues within the target's upstream dependency scope, without executing upstream nodes. Existing whole-document structural checks remain in place. Validation rejects the request before creating runs or executing nodes.

The public API responds with HTTP 400 and `code: "flow_validation_failed"`. Its `details` object contains:

```json
{
  "phase": "validation",
  "diagnostics": [{
    "node_id": "node-aggregate",
    "code": "variable_aggregator_output_mismatch",
    "field_path": "/outputs/0/title",
    "message": "variable_aggregator output title must match group result",
    "expected": "result"
  }]
}
```

When `node_id` is present, `field_path` is a JSON Pointer relative to that node, using standard `~` and `/` escaping. Without a node ID, it addresses the flow document. An empty pointer addresses the complete node or document; it does not imply a field location that the validator cannot determine. `expected` is included only when the rule knows a concrete constraint. Validators supply node IDs, stable codes, and paths directly; adapters do not reconstruct them from exception text. Public messages use the existing API sanitization rules.

MCP reuses the public API `details` projection as `target_code` and `target_details`. Authentication, authorization, and internal server details are not exposed through configuration diagnostics. GUI and MCP consume the backend contract rather than maintaining separate validators.

Pre-execution failures do not fabricate node runs. Failures after execution retain the existing persisted `node_run.error_payload`, state transitions, and terminal behavior. Internal failures remain HTTP 500. This change does not alter node rules, write policies, caching, or provider protocols.
