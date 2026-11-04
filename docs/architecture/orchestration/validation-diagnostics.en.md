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

Pre-execution failures do not fabricate node runs. Failures after execution retain the existing persisted `node_run.error_payload`, state transitions, and terminal behavior. Internal failures remain HTTP 500.

## Draft analysis and output contracts

`POST /api/console/applications/{id}/orchestration/analysis` accepts `{ "document": ... }`. It analyzes an unsaved document using the owning application's type, authorization and compilation context. It does not bootstrap editor state, create versions or execute nodes. Its Canonical Interface, `applications.orchestration.analysis`, is shared by the GUI and MCP wrappers.

The response contains `diagnostics`, `node_outputs` and `variable_catalogs`. Diagnostics and resolved outputs cover the whole document. The optional request `node_id` selects the only target whose catalogs are expanded; without it, catalogs are empty. This avoids materializing the entire graph's transitive closure in large workflows. Diagnostic levels are `error` or `warning`. Output-contract selectors are relative to the node result; variable-catalog selectors include the source node ID. Catalogs are grouped by target `node_id` and a node-relative JSON Pointer `field_path`. An empty path provides the default catalog; specific binding paths represent position-dependent scopes such as container updates. Catalogs expose declared public outputs and in-scope variables, never environment variable values.

Code execution retains the envelope `{ "result": objectReturnedByMain, "error": null }`. An explicit `outputs[].selector` is authoritative, including `["result"]` for the entire returned object. Only an absent selector defaults to `["result", key]`. For a `main` function returning `{body:"...",count:7}`, declare:

```json
{
  "key": "result", "title": "Bundle result", "valueType": "object",
  "selector": ["result"],
  "jsonSchema": {
    "type": "object", "additionalProperties": false,
    "properties": {"body":{"type":"string"},"count":{"type":"integer"}},
    "required": ["body","count"]
  }
}
```

Consumers may reference `["pack","result"]` or `["pack","result","body"]`. A business member also named `result` is explicitly addressed by `["result","result"]`. Historical declarations must not be migrated by guessing what a key meant.

The backend shares output-path and lexical-scope checks. Unknown properties of a closed schema and attempts to traverse a scalar are errors. Paths through open JSON remain usable with a warning, and their actual values must exist at execution. Local `$ref` references, schema composition and sibling property constraints are evaluated together. The frontend consumes backend catalogs rather than independently adding Code selector prefixes or undeclared HTTP outputs.

Save, import and restore reject malformed base structures, output selectors and schemas before persistence. Semantically incomplete drafts remain saveable, with `analysis` included in editor-state responses. Errors block publication and schedule enablement; warnings do not. Schedule enablement checks the current draft, active publication snapshot and actual compiled plan. Publication rechecks the frozen snapshot before activation. The existing freeze read/write interval does not provide transactional compare-and-swap guarantees.

Code without source/source_ref produces a readiness error at `/config/source`. After execution, a missing explicitly declared output path fails at its Code node. Optional and nullable object members use the existing JSON Schema `required` and type rules. Static analysis never runs JavaScript or proves that every business branch will return valid data; execution still validates the real result.
