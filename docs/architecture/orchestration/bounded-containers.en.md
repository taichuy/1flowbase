# Bounded loops and iterations

[中文](bounded-containers.md)

This document defines the container contract and its evidence boundaries. GitHub Root #2711 owns delivery status and acceptance; this document does not claim that verification has passed.

## Authoring

Loops and iterations display their children on the same canvas and may nest. Each container has one automatically created internal start. Edges stay within a scope; container inputs and outputs transfer values across scopes. Moving a parent preserves child-relative coordinates. Size, copying, deletion, undo and redo are part of the persisted document.

| Setting | Loop | Iteration |
| --- | --- | --- |
| Bound | Required positive integer `max_rounds`, initially 10 | A snapshot of `bindings.items`; an empty array runs zero rounds |
| Per-round deadline | Required positive integer `round_timeout_ms` | Same |
| Error handling | `error_policy`: `fail`, `break`, or `continue`; default `fail` | Same |
| State | `loop_variables`, initial `bindings.variables`, successful-round `bindings.variable_updates` | Isolated `item` and `index` per item |
| Condition | Optional `bindings.break_condition` | Array length determines the bound |
| Output | `result` array and final `variables`; optional `bindings.output` | Required `bindings.output`; successful values ordered by input index |
| Concurrency | Serial because state passes between rounds | User-configured positive `parallelism`; default 1 |
| Flattening | Preserve the explicitly selected output structure | `flatten_output` expands one level when enabled; disabled by default |

Users configure deadlines; defaults are not guessed from the development machine. Concurrency has no arbitrary business cap based on assumed client behavior. Integer, clock and semaphore representation limits still apply. An upstream Code node can supply an array without pretending that it is a file input.

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

Round deadlines persist as absolute times and are not renewed on recovery. Effective deadlines cannot exceed an ancestor deadline; process-local scheduling uses monotonic time. A code invocation's isolation budget is narrowed to its remaining deadline without changing the saved configuration.

A Tokio timeout does not forcibly stop every task. Verification distinguishes admission stopping, bounded local cleanup, and remote effects. A stopped service cannot promise an on-time database transition; recovery first handles expired work. Remote effects are not reported as rolled back merely because a local deadline expired.

The termination argument assumes finite leaf completion, finite scope DAGs and ownership depth, finite admitted rounds, progressing scheduling, and recovery that does not reset budgets. Structural induction over nesting depth then establishes finite execution in that model. It does not prove that the Rust implementation, database or remote services are fault-free.

## Evidence

Compiler counterexamples live in `orchestration-runtime/src/_tests/compiler_tests/containers.rs`. Runtime and independent finite-sequence fixtures live in `execution_engine/container/_tests/`. PostgreSQL fixtures cover atomic commits, stale owners, round output and application scope in `control-plane-postgres-tests/tests/orchestration_runtime_integration/_tests/`.

Frontend fixtures cover authoring, lexical selectors and trace identity; nested canvas transforms have separate fixtures. The authenticated browser scenario is documented in `scripts/node/page-debug/scenarios/bounded-containers/README.md`. It creates a real temporary application, edits and reloads the canvas, runs two rounds over three items, checks execution identities, and cleans up the application and session.

Enumeration bounds describe test domains, not product limits or a whole-program proof. Acceptance must report the candidate SHA, commands, actual results, original artifacts and unverified scope. Documentation, compilation and screenshots each establish only their own limited facts.
