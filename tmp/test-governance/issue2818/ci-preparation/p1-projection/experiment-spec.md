# #2818 P1 — finite synchronous compatibility SSE projection boundary
Date: 2026-11-10 (Asia/Shanghai)
Root: ChatGPT. Local Codex is an implementation assistant only.

## Frozen input and scope
Protected beta: 4fdf189bf6525fd7fc14d2ccf0a1a86a3c07c184, clean at entry.
Reuse temporary worktree /home/taichuy/git/git_worktree/issue2818-whitebox-owner-20261109 on a NEW branch based on this SHA. Preserve the old packet branch.
One intervention: move the existing OpenAI Chat, OpenAI Responses and Anthropic SSE projection state machines, their immediate pure projection dependencies, and concrete event view to api/apps/application-public-protocol. API owns ingress, auth, invocation, persistence, transport, callback reservation/execution and snapshot/generation lifetime exactly as before.
Do not migrate the whole application_public_api subtree.

## Hypotheses and disconfirming observations
H1 (strong stage evidence, unproven family attribution): API monomorphization reachability has a large resident working set before LLVM. Diagnostic 24b444/run42811084825 raw time-passes: collector RSS endpoints 3767836672 -> 7680368640 B. Ordinary ed53/run42801400662 raw API rustc rss reaches 7846322176 B and parent anon reaches 7976308736 B, then OOM kill. These are DIFFERENT source/flags and not a strict performance pair. Selected SSE mapper bodies occur in the original API mono emission; no name count is an RSS percentage.
H2 (direct counterexample): moving public definitions / opaque futures can leave their heavy concrete bodies in consumers. The withdrawn codec experiment moved 9 constructors but left 9 poll bodies in API. Falsify the chosen cut if migrated mapper core execution bodies remain instantiated in API.
H3 (direct separate whole-task risk): restored/retained cache file pages can saturate parent memory independently of API; ordinary API's near-zero file charge at peak rejects file cache as its sole cause. Keep these phases in the original denominator. This single source change does not claim to solve cache restoration peaks.

## Intended boundary
Expose concrete synchronous state-machine methods taking existing NativeRunResult and a concrete RuntimeEventEnvelope/view. No opaque async return, no new dyn dispatch, no Box, no shared global state, no new resource cache. If callers require generic Into convenience, keep only a tiny conversion shim; the full projection match and state update must be non-generic in the new owner.
Move (not copy) immediate shared pure helpers required by the three mappers only, retaining old module reexports for other actual consumers. No new protocol fields or semantics. Preserve Vec/event ordering, partial/terminal handling, usage deltas, errors and callback tool identities.
Expected residuals: transport stream futures, InterfaceRuntime invocation generic bodies, storage/service futures, Axum host router/middleware, DTO serializers used by other API consumers, public type drop glue. No claim all mentioned instances disappear.
Runtime expectation: no additional allocations, boxes, virtual calls, state owners, or change to object lifetime. Any discovered need for one returns to Root before implementation.

## Behavior and tests
Preserve all old assertion bodies, test attributes, roots/full names, ignored state, and consumer selectors. Test-only helpers may stay in their original API root; do not promote a whole test tree into production or add feature skips. Record any unavoidable source path move with original SHA256/root/full name -> new owner.
Affected contract: exact SSE event order/payloads, terminal suppression, usage, reasoning/text distinction, errors, tool callback identity. Authentication, state/persistence/side effects, stream cancellation/recovery, frozen snapshot/generation ownership remain at original implementations and are protected by unchanged original integration tests.
F0/AC-009 and original 17-stage workload remain mandatory. Source maps, new crate compilation and local projection tests are not full behavior acceptance.

## Experiment and acceptance
First source-preserving prototype only. Root reviews exact diff and builds/measurements before accepting a full migration.
Use existing CI instrumentation/runner. Fixed 8000000000 B, swap0, memory.high=max, CPU<=60% of runner, single heavy task; retain original jobs1/profile test/features test-support/600s ordinary API command.
Ordinary command: cargo build --profile test -p api-server --features test-support --lib --jobs 1 --message-format=json.
Use saved matching baseline evidence wherever valid. Local default stable1.99.0 and memory.high7199997952 differ from CI rustc1.99.1/high=max: no cross-environment exact delta.
Mechanism diagnostic uses separate target/caches and exact recorded rustc argv; no diagnostic artifact is a normal-build acceptance artifact.
Success to continue: explicit body ownership migration AND ordinary API/owner each complete under unchanged budget, resource data supports useful API reduction without new peak; final integration requires full original same-range resource/test gates incl task peak<=6400000000 B.
Stop/rollback: body remains API; behavior/assertion drift; no identifiable API benefit; ordinary build OOM/deadline; any new peak or fixed full-task gate fails. OOM baseline is incomplete: no full build speedup or true peak reduction percentage.
A negative result is a valid conclusion. Do not expand to another domain.

## Implementation packet P1-source
Owned: new app crate; API compat_sse protocol_mappers and concrete view definitions; immediate pure shared projection helper files (usage/tool identity/presentation classifier) only where required; Cargo workspace+API dependency+lock; API/crates ownership and dependency boundary test/normal selector registration needed for new package.
Forbidden: profile/features values, allocator, concurrency, budget, test assertions/skips, control-plane business/state/persistence behavior, service state, unrelated global optimization packets, runtime deployment.
No builds/CI/push/merge/service actions delegated. Report exact changed files and unverified obligations. Root retains diagnosis, experiment design, acceptance and integration.
