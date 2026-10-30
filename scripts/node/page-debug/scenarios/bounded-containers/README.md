# Bounded container browser acceptance fixture

F03 repairs the QA1 fixture failures and extends the already-approved UI evidence. The repaired scene is **authored, not executed**. QA1's resize failure remains a failed result; syntax checks do not replace a new central QA run. This command starts no service and never edits an existing application.

## Run

From the assembled candidate worktree, against its already-running services:

```bash
node scripts/node/page-debug/scenarios/bounded-containers/run.cjs \
  --web-base-url http://127.0.0.1:3102 \
  --api-base-url http://127.0.0.1:7802 \
  --chrome /usr/bin/google-chrome \
  --reference-manifest /absolute/path/to/private-reference-manifest.json \
  --output-dir tmp/test-governance/bounded-containers-browser/qa2
```

Use `--web-base-url http://127.0.0.1:3100` and its matching API origin for the final local surface. Neither service is started, stopped, or taken over. `--headed` is optional. `PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH` is accepted when `--chrome` is omitted. Use an empty evidence directory for each run.

Dependencies: installed repository Playwright, local Google Chrome, migrated API/database, the assembled container compiler/runtime/persistence/trajectory fixes, published loop/iteration catalog entries, and the existing JavaScript runtime. No model provider, external request node, plugin, or live scheduled execution is required.

Credentials come from `loadRootCredentials` and `openTemporaryOwnerSession` in `scripts/node/page-debug/auth.js`. Cookie and CSRF values remain in memory. No password argument, direct sign-in, storage-state dump, HAR, or request headers are recorded.

## Original user image references

A private manifest supplies the three original PNG attachments, without putting personal absolute paths in source control:

```json
{
  "references": [
    { "role": "canvas-overview", "path": "/absolute/private/original-canvas.png" },
    { "role": "loop-settings", "path": "/absolute/private/original-loop.png" },
    { "role": "iteration-settings", "path": "/absolute/private/original-iteration.png" }
  ]
}
```

The scenario copies the originals byte-for-byte to `references/`, records SHA-256 and dimensions, and writes `visual-comparison.md` mapping each original to its candidate screenshots. It does not claim visual equivalence automatically. QA must view both images and record the white rounded container, cyan icon, pale dotted body, internal start/edges, blue selection, inspector hierarchy, and any material differences. Deadline controls are approved additions; fixture node types, app chrome and graph topology are expected differences. The original references must not be replaced with newly captured candidate images.

## Real scenario and assertions

1. Create a uniquely named temporary Workflow and immediately disable its schedule. Never publish it. Prepare `Workflow Start → Code → Workflow End` through the real draft API. The existing paragraph/string input `items_json` carries `"[1,2,3]"`; Code parses it to its declared `result.items` array. No new Start input type is introduced.
2. Use the real `BC Input` plus picker to create a Loop, configure it through the inspector, save, and require exactly one persisted automatic `container_start`. Preserve that real Loop/start pair when preparing the remaining graph.
3. Prepare three actual nested containers: **Loop(2) → Iteration(3) → Loop(1) → Template Transform**. Iteration exports the innermost loop's one-element result and uses the existing one-level flatten option. Thus the original 2 × 3 output remains unchanged and the leaf still executes exactly six times.
4. Through real fields, set outer maximum rounds 2, timeout 30000 ms and failure strategy `continue`; set iteration timeout 15000 ms and strategy `break`. Use the real array Cascader to select `BC Input / result / items`. The innermost loop has one round and a 5000 ms fixture deadline. These are explicit fixture inputs, not product defaults.
5. Select each container layer and the leaf, verifying the matching inspector alias and retained root canvas. Before moving or resizing, read `elementFromPoint` at the actual control, pan the real canvas with the middle mouse button when occluded, and verify the resulting hit. `receipt.pointer_targets` retains the targeting evidence. Never use forced clicks or DOM relocation to bypass a dock.
6. Move and resize the outer container, save, and require unchanged descendant-relative positions, increased dimensions, and unchanged execution semantics. Copy the entire three-layer subtree through actual `Ctrl+C`/`Ctrl+V`; inspect its saved graph for fresh IDs, internal selector/template remaps, retained external array reference, and a single root offset. Use real Delete, Undo, and Redo, saving each state; require exact subtree restoration and final equality with the original graph. Do not fit/pan between history steps, because viewport changes are legitimate editor history.
7. Reload and require graph equality. Use the formal Test Run form with `{"items_json":"[1,2,3]"}`. Click its actual `button[type="submit"]`, accommodating Ant Design's displayed `运 行`. The response listener has an immediate rejection handler and is always removed/settled when the action fails, so teardown cannot cause an unhandled response-promise rejection.
8. Require `flow_run.status === "succeeded"`, output `{"results":[["0:1","0:2","0:3"],["1:1","1:2","1:3"]],"after":"done"}`, a real Code-produced array, and one successful root successor. Persist the restricted run response before assertions so a failed execution remains diagnosable.
9. Require six distinct leaf `node_run.id` values and six distinct canonical `execution_identity.execution_id` values. These are separate identifiers, not falsely required to be equal. Require three-element `round_path` values `0/0/0`, `0/1/0`, `0/2/0`, `1/0/0`, `1/1/0`, `1/2/0` with the correct container IDs and nonempty parent/execution IDs. Never infer identity from debug text.
10. Open the formal execution Trace, select the nested `1/1/0` round through its real TreeSelect, and require the rendered BC Item rows to point to the corresponding node-run receipt with all three path labels. Click a row and verify its selected state.
11. Capture desktop overview, three inspector levels, copy/history, reload, run and selected trajectory screenshots. In a separate 390 × 844 context, wait for the formal editor data to load, measured canvas nodes and the actual application title; then capture the existing surface and measure overflow. Do not fit/pan the desktop graph or require inner nodes to be visible on the phone to manufacture acceptance. A loading shell does not satisfy readiness.

`page.evaluate` only reads geometry, animation stability, and rendered evidence. There is no route mock, DOM injection, application store manipulation, or response substitution. API preparation is distinct from actual UI creation/configuration/history/run assertions.

## Contracts and evidence boundaries

The API contracts are `console/applications.ts`, `application-public-api/index.ts`, `console/application-orchestration.ts`, and `console/application-runtime/types.ts`. Workflow test input is built by `features/workflow/lib/test-run.ts` and `WorkflowTestRunPanel.tsx`. Code output uses its established `['result','items']` selector. Trajectory assertions consume the typed `execution_identity` (`execution_id`, `node_id`, `parent_execution_id`, `round_path`) and zero-based round indices.

This scene checks that selected error policies persist, not their failure semantics. Backend fixtures own cancellation, error matrices, recovery, external side effects, and concurrent stop points. Six valid UI-run records are not proof of those properties.

## Artifacts and cleanup

`receipt.json` records the candidate SHA, origins, Chrome version, owned application/run IDs, assertion results, pointer targeting, video paths and original-image hashes. `videos/` contains real desktop and mobile recordings, finalized when contexts close. Saved document snapshots cover creation, copies, final history state and reload; the restricted run artifact includes status, error, outputs and canonical identity. Original comparison status remains pending until independent visual review.

In `finally`, close contexts to flush video, close the browser, cancel a nonpassing run when its ID is known, delete only the created application ID, revoke the owned session, and require HTTP 401 from the old session. `cleanup.json` records results; cleanup errors fail the scenario. Evidence is intentionally retained. No transient credential file is created. Forced process termination outside JavaScript cleanup may require the QA owner to remove the exact application ID recorded in the receipt; never delete an existing app by display name.

The related frontend regression batch includes the formal Workflow registry import and field-aware scope positive/negative assertions. It no longer relies on the obsolete helper-name string `listAuthoringVariableOptions` to prove the approved variable boundary. Run those tests only in the Root's consolidated QA batch.

## Mobile evidence boundary

DESIGN.md §10.1 requires a summary and desktop-operation guidance at widths up to 768 px. Static inspection found that `WorkflowEditorPage` unconditionally renders `WorkflowEditorAssembly` after its data queries, and `FlowEditorKernel` renders its canvas slot without a viewport branch. Both owners already had that behavior at baseline `6bdb87495`; the current local media rule adjusts picker/control widths, not the page strategy. There is no implemented mobile summary/guidance branch to validate in this scope.

The mobile receipt therefore records `observed_strategy: desktop_canvas_without_mobile_summary` and `design_compliance: known_existing_deviation`. Its ready assertion proves only that the screenshot is of the actual loaded editor rather than a loading shell. Overflow is an observation, not a mobile-design PASS. The scenario does not expand this task into a mobile redesign or modify product behavior to make phone canvas assertions pass. Independent QA must keep this design gap explicit even if the desktop and run assertions pass.

The internal start icon now follows the original visual reference: the existing 40 px white circle contains a 24 px `colorInfo` blue circle with a white Home icon. Its source handle, picker and selection behavior are unchanged; this small style change still needs the consolidated visual check.
