# Bounded container browser acceptance fixture

This scenario is **authored, not executed** in P07a. It is a candidate-bound fixture for the Root #2711 central QA batch. It starts no server, installs no dependency, and never changes an existing application.

## Run

From the candidate worktree, against its already-running web/API services:

```bash
node scripts/node/page-debug/scenarios/bounded-containers/run.cjs \
  --web-base-url http://127.0.0.1:3102 \
  --api-base-url http://127.0.0.1:7802 \
  --chrome /usr/bin/google-chrome \
  --output-dir tmp/test-governance/bounded-containers-browser/candidate-01
```

For the final local web surface, pass `--web-base-url http://127.0.0.1:3100` and the actual matching API port. Neither port is restarted or taken over. `--headed` is optional. `PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH` is accepted when `--chrome` is omitted. The evidence directory must be empty; use a new directory for each candidate/run.

Dependencies: repository `web/node_modules` Playwright, installed local Google Chrome, migrated API/database, authenticated console and Workflow APIs, the container compiler/runtime/persistence/log projection changes assembled, and an exposed loop/iteration node catalog. The existing JavaScript code runtime must be available. No model provider, external network request node, plugin, or scheduler execution is required.

Credentials come from `loadRootCredentials` in `scripts/node/page-debug/auth.js` and its existing service environment. The only login owner is `openTemporaryOwnerSession` from that module. Cookie/CSRF values remain in memory and are never written to evidence. Do not pass a password on this scenario's CLI.

## Fixture and real interactions

The scenario creates a uniquely named temporary Workflow application and immediately disables its schedule trigger. It never publishes the application. Test Run uses the existing schedule test form, which explicitly does not enqueue the real schedule.

1. Prepare a small `Workflow Start → Code → Workflow End` draft through the real draft endpoint. `items_json` is an existing **paragraph/string input**, not a new array input type. Code uses `JSON.parse` to produce its declared `result.items` array.
2. Use the real canvas `BC Input` plus picker to create a Loop. Set alias, maximum rounds and timeout through the inspector, save, and read the server draft. Require exactly one persisted automatic `container_start`.
3. Prepare the remainder around that same UI-created Loop ID: `Loop → Iteration → Template Transform`. Keep its UI-created start. Add a root template successor and Workflow End. The preparation leaves the iteration's items binding unset.
4. Through the real inspector set outer rounds to 2, outer timeout to 30000 ms and error policy to `continue`; set inner timeout to 15000 ms and policy to `break`. Select `BC Input / result / items` in the real array variable Cascader. These values are fixture inputs, not new product defaults.
5. Select the inner leaf and require its inspector alias while the root node remains on the same canvas. Drag the outer header and resize its real xyflow control. Save and verify root movement, unchanged child-relative coordinates, increased dimensions, and exact persisted graph equality after reload.
6. Use the formal Test Run drawer with `{"items_json":"[1,2,3]"}`. Read the actual POST response and debug snapshot. Open the formal execution Trace page.
7. Capture desktop creation, inner selection, saved/reloaded canvas, run result, Trace, and a 390 × 844 mobile editor screenshot. Mobile evidence covers layout/overflow only; it does not claim mobile canvas editing.

No browser route mock, DOM injection, application store manipulation, or response substitution is used. `page.evaluate` is restricted to reading geometry/animation stability. API preparation is separate from the subsequent UI assertions; it does not replace the automatic-start/picker proof or field/move/resize saving.

## Assertions and contract sources

| Assertion | Source contract |
| --- | --- |
| Application create/delete and disabled schedule trigger | `web/packages/api-client/src/console/applications.ts`, `application-public-api/index.ts` |
| Draft save body `{document, change_kind, summary}` and returned `draft.document` | `console/application-orchestration.ts` |
| Test Run input `{input_payload:{[startId]:{items_json:"[1,2,3]"}}, document}` | `features/workflow/lib/test-run.ts`, `WorkflowTestRunPanel.tsx` |
| Success status and results `[["0:1","0:2","0:3"],["1:1","1:2","1:3"]]`, successor `done` | `ConsoleApplicationRunDetail.flow_run.{status,output_payload}` and explicit fixture templates |
| Code produced the actual `[1,2,3]` array | Code output contract selector `['result','items']`; `node_runs[].output_payload.result.items` |
| Six leaf records, distinct record IDs and expected 2 × 3 paths | `ConsoleNodeRunDetail.execution_identity` from P06: `{execution_id,node_id,parent_execution_id,round_path}`; each path element `{container_execution_id,container_node_id,round_index}` |
| Record identity is the execution identity | `node_runs[].id === execution_identity.execution_id`; never inferred from debug payload/text |

The exact P06 typed identity field is required; missing fields fail instead of silently skipping the assertion. `round_index` is zero-based. Outer paths are 0/1 and inner paths 0/1/2. A successful root successor must execute once after the containers.

The fixture checks that chosen error policies persist. It does not claim to prove their failure behavior, cancellation, recovery, parallel stop points, or the whole backend matrix; those are owned by backend fixtures. The API projections and screenshots are not substitutes for that evidence.

## Evidence and cleanup

`receipt.json` records source SHA, service origins, Chrome version, created application/run IDs, pass/fail and scoped assertions. JSON artifacts hold the known fixture drafts and a restricted run projection (status, outputs, typed execution identity). Screenshots are retained. No HAR, request headers, cookie file, browser storage dump or credentials are recorded.

In `finally`, close browser contexts, cancel a nonpassing run when its ID is known, delete only the created application ID, revoke the temporary session through its owner, and check that the old session returns HTTP 401. `cleanup.json` reports each result; cleanup failure makes the scenario fail. Evidence files are intentional retained outputs; no transient credential files are created. If the process is forcibly killed outside JavaScript cleanup, use the exact `application_id` from the receipt to clean up that fixture; never select an existing application by display name.

Known integration risk identified while authoring: Workflow's separate variable validator originally used only flat upstream traversal. Central assembly must include its container-scope update. This scenario does not suppress Issues or fake a selector to avoid that risk. The array source deliberately uses a normal Code node so Start input types remain unchanged.
