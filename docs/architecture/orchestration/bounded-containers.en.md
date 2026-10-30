# Bounded loops and iterations

[中文](bounded-containers.md)

This document defines the container contract and its evidence boundaries. GitHub Root #2711 owns delivery status and acceptance; this document does not claim that verification has passed.

## Authoring

Loops and iterations display their children on the same canvas and may nest. Each container has one automatically created internal start. Edges stay within a scope; container inputs and outputs transfer values across scopes. Moving a parent preserves child-relative coordinates. Size, copying, deletion, undo and redo are part of the persisted document.

| Setting | Loop | Iteration |
| --- | --- | --- |
| Bound | Required positive integer `max_rounds`, initially 10 | A snapshot of `bindings.items`; an empty array runs zero rounds |
| Per-round deadline | Optional positive integer `round_timeout_ms` (milliseconds); missing / null means no local round deadline | Same |
| Error handling | `error_policy`: `fail`, `break`, or `continue`; default `fail` | Same |
| State | `loop_variables`, initial `bindings.variables`, successful-round `bindings.variable_updates` | Isolated `item` and `index` per item |
| Condition | Optional `bindings.break_condition` | Array length determines the bound |
| Output | `result` array and final `variables`; optional `bindings.output` | Required `bindings.output`; successful values ordered by input index |
| Concurrency | Serial because state passes between rounds | User-configured positive `parallelism`; default 1 |
| Flattening | Preserve the explicitly selected output structure | `flatten_output` expands one level when enabled; disabled by default |

There is no default round timeout. Missing or null creates no local round deadline; explicit positive integer milliseconds remain compatible. Zero, negative numbers, fractions and strings are rejected. The UI edits minutes while the API and persisted field remain `round_timeout_ms`. Concurrency has no arbitrary business cap based on assumed client behavior. Integer, clock and semaphore representation limits still apply. An upstream Code node can supply an array without pretending that it is a file input.

## Execution

For a container invocation with frozen limit N and started-round count s, maintain `0 ≤ s ≤ N`. A new round is admitted only when `s < N`; its identity and the increment commit together. Resuming the same round neither consumes another round nor resets the count. Ownership is finite and acyclic, and each scope remains a DAG.

A loop checks its condition after initialization and after each successful variable commit. An initially satisfied condition permits zero rounds. A condition error after the final successful round is a separate control fact: it preserves the successful round and consumes no additional budget. Fail ends the workflow; break/continue leave the exhausted container while retaining the error.

- `break` exits the current container, preserving committed successful results; the enclosing workflow continues.
- `continue` consumes the failed round and starts the next with the last successfully committed loop variables.
- `fail` terminates the workflow; an enclosing continue policy cannot absorb it.
- Existing node error branches run before the container policy. Cancellation and an expired ancestor cannot be absorbed by a local continue policy.

Parallel break uses a durable stop point. Successful outputs committed before it are retained; no new rounds start afterward. Late results cannot change sealed output. Partial results need not form a contiguous input prefix. Nested scopes share a finite execution budget, and a parent waiting for children must not occupy all workers needed by those children.

## Identity, scope and recovery

A static `node_id` identifies the canvas definition. A stable logical `execution_id` identifies one execution, with `parent_execution_id` and a `round_path` of container executions and original round indexes. Its `node_run_id` identifies the durable receipt in the current flow run. A native recovery successor may preserve logical identity while owning a different receipt UUID.

Consumers use the backend's `execution_identity`, never debug text. One state owner coordinates node facts, round progress and the current recovery position. Historical facts stay in the database. Checkpoints retain active recovery state; completed rounds release local execution state, while contractually required aggregate results remain. Long-lived history is not stored in ephemeral caches; existing short-lived published-plan caching serves a different purpose.

Root successors also persist admission, completion and progress. Claim generations fence stale owners. Concurrent provider calls use per-future execution identity and the corresponding receipt, rather than a shared current-node slot. LLM nodes inside a container finish as intermediate round results; root final LLMs retain the existing communication-finalization behavior.

Confirmed durable completions are not replayed. An external request may have succeeded before its local completion was recorded. Without an existing idempotency or result-query contract, uncertain writes are not automatically replayed. This is not an exactly-once guarantee for remote effects. Retrying observation persistence must not duplicate spans, context references or usage facts.

## Time and termination

Optional absolute round deadlines and their owners persist without renewal on recovery; historical numeric checkpoint deadlines still decode as finite values. The earliest finite local or ancestor deadline wins: finite parents constrain unlimited children, and unlimited parents allow finite child deadlines. Without a finite deadline, execution creates no timeout timer and waiting/recovery invents no deadline. Database NULL means unlimited; updates may tighten NULL to finite or shorten a finite deadline, but cannot clear or extend a finite deadline. Finite process-local deadlines use monotonic time. Code isolation budgets narrow only when an effective round deadline exists; otherwise the node's own timeout remains intact. Expiry cancels the round, records timeout and applies the existing fail / break / continue policy. Round counts and input-array bounds remain unchanged.

A Tokio timeout does not forcibly stop every task. Verification distinguishes admission stopping, bounded local cleanup, and remote effects. A stopped service cannot promise an on-time database transition; recovery first handles expired work. Remote effects are not reported as rolled back merely because a local deadline expired.

An unlimited round does not guarantee wall-clock termination. The termination argument assumes finite leaf completion, finite scope DAGs and ownership depth, finite admitted rounds, progressing scheduling, and recovery that does not reset budgets. Structural induction over nesting depth then establishes finite execution in that model. It does not prove that the Rust implementation, database or remote services are fault-free.

## Evidence

Compiler counterexamples live in `orchestration-runtime/src/_tests/compiler_tests/containers.rs`. Runtime and independent finite-sequence fixtures live in `execution_engine/container/_tests/`. PostgreSQL fixtures cover atomic commits, stale owners, round output and application scope in `control-plane-postgres-tests/tests/orchestration_runtime_integration/_tests/`.

Frontend fixtures cover authoring, lexical selectors and trace identity; nested canvas transforms have separate fixtures. The authenticated browser scenario is documented in `scripts/node/page-debug/scenarios/bounded-containers/README.md`. It creates a real temporary application, edits and reloads the canvas, runs two rounds over three items, checks execution identities, and cleans up the application and session.

Enumeration bounds describe test domains, not product limits or a whole-program proof. Acceptance must report the candidate SHA, commands, actual results, original artifacts and unverified scope. Documentation, compilation and screenshots each establish only their own limited facts.

### Finite verification domains and rerun entry (F04)

The following table declares fixture domains, not passing results. Record the candidate SHA and run `cargo test --manifest-path api/Cargo.toml -p orchestration-runtime container::tests::evidence -- --nocapture` through the repository's Cargo budget entry. Collect `container_model`, `container_tree`, `container_trace`, `container_stop`, `container_clock`, `container_callback`, `container_quickjs`, and `container_cost` receipts; keep a single heavy process.

| Decomposed domain | Declared range | Evidence and limitation |
| --- | --- | --- |
| Independent state graph | N=0..3 × p=1/2 × three policies: 24 configurations; running/waiting/expired/terminal rounds, at most one crash and generation 0→1, one old-result observation per round | Standard-library BFS exhausts its queue without a truncation cap. Reports actual visited states, transitions, terminals and deadlocks. The first safety violation is a shortest BFS counterexample; nonterminal cycles are detected separately. |
| Normal nested trees | Depth 0..3 and effective count 0..3 at each level: 1+4+16+64=85 chain trees, alternating loop and iteration | Actual engine output is compared with an independent recursive tree. Zero-round loops use an initial condition, never max_rounds=0. |
| Serial failure sequences | Length 0..3, binary success/failure and three policies: 45 cases | Existing independent fold; filter `finite_error_sequences_match_independent_reference`. This is not concurrent-interleaving exhaustion. |
| Controlled bad producers | Reset budget, swallow global failure, accept stale generation, plus nonterminal spin | Valid models, normalized actual-engine traces and bad variants use the same verifier. Rejected traces and counterexample paths replace hand-computed count inequalities. |
| Time and stopping | p=1/2, success before/after break: four barrier orders; virtual ancestor deadline; three policies × callback-first/timeout-first/commit crossing: nine orders plus an absolute-deadline ancestor callback case | Active deadlines use Tokio virtual monotonic time. Persisted absolute deadlines use real short timers and admission barriers. Tokio advance is not claimed to advance UTC or the PostgreSQL clock. |
| Isolation and cost | Real QuickJS `while(true)` through the narrowed container budget; depth3/count2/p2 with 0/64KiB unrelated root data | The test retains and joins a runner-observation task, proving that the actual runner returned timeout. Reports peak leaf tasks, frames, local-value bytes, 15 logical execution records and 14 round commits; no remote-stop claim. |

Termination of the finite graph assumes callback/deadline events are eventually handled, at most one owner transfer, and no infinite idle/crash insertion. Exploration includes every legal successor, not random samples. The spinning producer must expose a nonterminal cycle. Cycle output is an actual closed walk; only safety counterexamples are claimed shortest, not arbitrary cycle lengths globally.

These are separate domains, not a complete nesting×failure×parallelism×time×recovery cross-product. Model and test-owner generations do not prove database fencing. Real locks, leases, external receipts and process termination remain PostgreSQL/F05 evidence. Reuse the actual isolation tests `code_runner_quickjs_timeout_is_stable` and `code_isolation_profile_timeout_is_enforced_by_quickjs_runner`. Type checking, fixture counts and declared combinations are not behavior passes.
