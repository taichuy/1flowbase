# API compilation audit — Root #2824

This is the checked-in method and initial report. The protected source is
`dc1aa27f9658b2aaf17541a23c8a76db77016f53`; the initial source snapshot below is
`ea36151bee5f246b5dad1ddcb74aa5af2bb8a661` (through D5-P2). Final frozen candidate
SHA and measurements are supplied to the committed generator as evidence artifacts.
This document does not assert final AC-003/004/005/009 or full QA acceptance.

## Workloads and measurement scopes

Run `scripts/node/compile-memory-audit/audit.py` with an explicit real workspace,
exact source SHA, role and new output directory. Export it from its committed source
before temporarily checking out protected beta in the single task worktree. The
export contains only the five audit files and the original resource-job, heavy-job
and budget-context helpers with source SHA/file hashes. It copies no .env, credentials,
local configuration contents or caches. Helper imports resolve within the export;
the existing source `verify-runtime.js` resolves the unchanged local jobs2/threads2
configuration in either beta or candidate. Main beta is never switched.

| Workload | Exact command from `workspace/api` | Jobs | Cache / role |
| --- | --- | ---: | --- |
| `dev2` | `cargo build --locked --offline -p api-server` | 2 | ALL default-feature bins, fresh target; baseline B1 once, candidate once |
| `dev10` | same ALL-bin build | 10 | candidate-only explicit measurement parameter, fresh target |
| `test2` | `cargo test --locked --offline -p api-server --no-run` | 2 | candidate-only fresh target |
| `incremental` | `cargo build --locked --offline -p api-server --bin api-server` | 2 | reuse completed dev2 target; exact-bin cached warmup then original local get_session mutation |
| `external` | explicit original command, from workspace root | source command's original configuration | reuse owned test2 target for additional workspace/domain lib-test compilation and full backend instrumentation |

Each cold workload is n=1. One absolute campaign file binds source/tool identity and
four immutable cold slots: baseline/dev2 plus candidate/dev2, dev10, test2. A failed
or unfinished slot is still claimed. Inspect its original controller PID, unit and
resource-job started/command-started/finished receipts; absence of a terminal receipt
is not permission to resend. No automatic retries or candidate threshold on baseline.

The wrapper preserves the original eight GNU time TSV fields: maximum RSS KiB,
wall/user/sys seconds, crate, kind, source and features. This is a **rustc invocation
and descendant maximum**, not direct rustc PID RSS or linker PID RSS. No `-Z`,
compiler/linker options, profiles or user parallel settings are changed. Toolchain,
resolved profile declarations, source/manifest/lock identities and configuration
hashes are recorded; configuration contents are not exported. Cold means a fresh
Cargo target, with OS page cache and registry/download caches preserved.

The original one-second `memory.stat anon` method is retained. A separate process
sampler records real PID/starttime/executable/cgroup, RSS/VmHWM and compiler ancestry.
It distinguishes rustc, cc/gcc/clang/collect2 drivers and actual ld/lld processes.
Short-lived or late peaks may escape sampling. Original bin-invocation descendant
RSS remains a separately labeled conservative upper bound for its linker; it is
never relabeled as direct linker RSS. Candidate validation requires all observed
rustc invocation bounds ≤4 GiB, dev2 anon ≤7.5 GiB, test2 anon ≤8 GiB, and dev
api-server bin-invocation linker upper bound ≤3.9 GiB. If that conservative bound
cannot establish the linker limit, the AC remains unestablished, rather than
claiming a sampled value is a true exit peak.

Whole workload CPU is exactly one wait4 user+sys scope for the principal and its
reaped descendants. Never sum Cargo, nested rustc, linker or cgroup CPU observations.
The sampler runs in the same original task budget, so its anonymous overhead is
included consistently in both new before/after measurements. Original Scout
exact-bin samples remain separate from the new canonical ALL-bin B1 comparison.

Target volumes use lstat for every entry including directory metadata, unique
(device,inode) accounting for hardlinks, and no traversal of symlinks. Apparent and
allocated (`st_blocks*512`) bytes are recorded, plus each actual produced top-level
bin and test/bin harness executable's sizes and SHA256. Record these before cleanup.
Only `cargo clean --target-dir <ownership-bound target>` may clean a measured target;
never delete target internals or user caches manually. Reuse has no silent cold
fallback. All controllers/workers use the existing issue2824.slice identity and
limits, original global heavy lock, whole-machine Cargo uniqueness and ≤90%
CPU/physical-RAM/disk admission; resource violations cancel only owned work.

## Export, B1 and candidate recipe

Use absolute paths. `TOOL_SHA` is the assembled commit containing these tools;
`CANDIDATE_SHA` is the final frozen source. `WORKSPACE` is the existing isolated
worktree. `BUNDLE` is a new directory outside it; `RAW` is an existing ignored or
external artifact parent. The original task budget declaration remains:

```bash
export ONEFLOWBASE_TASK_BUDGET_CGROUP=/user.slice/user-1000.slice/user@1000.service/issue2824.slice
export ONEFLOWBASE_TASK_BUDGET_ID=14758159
python3 "$WORKSPACE/scripts/node/compile-memory-audit/audit.py" export \
  --workspace "$WORKSPACE" --source-sha "$TOOL_SHA" --destination "$BUNDLE"
```

Root alone temporarily checks out protected beta in that worktree after the tool
commit is assembled and no developer/Cargo is active. Root saves/restores exact
assembly identity. Do not change the main worktree. Execute B1 once:

```bash
python3 "$BUNDLE/audit.py" run --workspace "$WORKSPACE" \
  --source-sha dc1aa27f9658b2aaf17541a23c8a76db77016f53 \
  --role baseline --workload dev2 --campaign "$RAW/campaign.json" \
  --output "$RAW/before-dev2" --deadline-seconds 2400
```

`run` creates a controller scope in the existing task slice and invokes the exported
original resource-job and heavy-job helpers. Retain original CLI handle and every
receipt. Baseline >4 GiB rustc is recorded, not rejected by candidate limits. Before
restoring assembly, clean only after the measurement/volume receipt exists, under a
new bounded original wrapper invocation:

```bash
systemd-run --user --scope --slice=issue2824.slice --unit=issue2824-before-clean \
  python3 "$BUNDLE/resource-job-run.py" --deadline-seconds 180 \
  --output-dir "$RAW/before-clean-job" -- \
  python3 "$BUNDLE/resource-heavy-run.py" python3 "$BUNDLE/audit.py" clean \
  --workspace "$WORKSPACE" --source-sha dc1aa27f9658b2aaf17541a23c8a76db77016f53 \
  --output "$RAW/before-dev2/data"
```

### Missing cache marker: finite cleanup-only exception

[Cargo #12441](https://github.com/rust-lang/cargo/issues/12441) reports that Cargo
does not create `CACHEDIR.TAG` when the target directory already exists, even when
empty. The unchanged audit `owned_target()` creates its fresh directory first;
local Cargo 1.99 subsequently rejected both full and package cleanup with “missing
or invalid CACHEDIR.TAG”. B1's completed build, original volume/bin hashes and
successful job receipts remain valid. Do not rebuild B1, claim another cold slot,
change the five-file measurement bundle, or change its initial target/cache state.

Only after completed Cargo execution and volume/hash capture, the standalone
`scripts/node/compile-memory-audit/cache-tag.py` may add the missing cache metadata.
This is the sole task-specific exception to manual target writes. It validates the
real clean workspace/source, manifest/lock, original exported tool hashes, fresh
ownership and corresponding managed job/process receipts. Current
volume fields other than allocated bytes and every originally recorded bin/harness
device/inode/apparent size/SHA must match exactly. Physical allocated bytes are
accounted separately: deduplicate recorded artifacts by (device,inode), calculate
each original/current allocated-byte difference, and require their exact sum to
equal the entire target's allocated-byte difference. Unknown differences, conflicting
hardlink snapshots or any content/other volume change are refused; there is no
tolerance or force option. The external repair record preserves both volumes,
per-inode original/current allocations, alias paths and the exact sums without
changing historical measurement values or formal report statistics. B1's observed
4096-byte allocation-only change has no established cause; this method does not
attribute it to delayed allocation or another filesystem mechanism.
The historical root directory inode was not recorded: the repair records its
current identity and pins that directory with a dirfd/NOFOLLOW during validation
and exclusive creation. No unmeasured/unknown target, symlink, changed content or
existing marker (including an invalid marker) is accepted; no force/overwrite or
compiled-file deletion is provided. The helper runs no Cargo and requires the
original actual task budget, no global Cargo and <=90% resource admission.

Cleanup eligibility is separate from measurement acceptance. The original
all-checks-pass path still requires managed command/runner exit0. A candidate whose
Cargo process exited0, with no error/cancellation/OOM and complete process/job
cleanup, may also qualify when only the known memory checks failed and its exact
original command/runner exit is2. The complete workload-specific check dictionary
must contain only boolean values: workload_completed, rustc_records_present when
emitted, and source_and_configuration_unchanged must all be true. The current
candidate memory keys are all_observed_rustc_invocations_at_most_4_gib,
observed_direct_rustc_hwm_at_most_4_gib, anonymous_peak_limit and
api_server_link_descendant_upper_bound_at_most_3_9_gib, in the exact subsets emitted
by audit.checks for each workload. This exception requires candidate role and
candidate_thresholds_applied=true; unknown/missing keys, safety failures, Cargo
failure or any exit/check contradiction refuse cleanup. Repair evidence records
the original gate exit and failed memory keys. Original checks, thresholds, FAIL
verdict and measurements remain unchanged; cleanup grants no QA acceptance and
does not authorize a cold-build retry. Retain failed measurement/QA receipts and
use NEW independent repair and Cargo cleanup job/record paths.

Export only this committed stdlib helper separately as `CACHE_TAG_HELPER`; keep
`BUNDLE` and its original export hashes untouched. `DATA` names the original fresh
owner's data directory (for B1, `$RAW/before-dev2/data`). Record hashes of all original
non-target raw artifacts and their pre-metadata target volume first. Set
`OBSERVATION_DATA` to the explicitly selected latest existing observation on that
same ownership chain: use `DATA` for an unchanged cold target; after dev2 incremental
use its incremental data directory; after test2 workspace/full-backend work use
the most recent external data directory. Incremental selection validates the
existing mutation measurement against `incremental.json`, successful original job,
source_restored_clean and exact committed handler restoration. No historical
measurement is edited or manufactured to accommodate an unobserved change.

Run the helper under a NEW bounded original managed invocation, with repair records
outside every original data directory and tool export:

```bash
systemd-run --user --scope --slice=issue2824.slice --unit=issue2824-before-cache-tag \
  python3 "$BUNDLE/resource-job-run.py" --deadline-seconds 180 \
  --output-dir "$RAW/before-cache-tag-job" -- \
  python3 "$BUNDLE/resource-heavy-run.py" python3 "$CACHE_TAG_HELPER" \
  --workspace "$WORKSPACE" --source-sha dc1aa27f9658b2aaf17541a23c8a76db77016f53 \
  --data-dir "$DATA" --observation-data-dir "$OBSERVATION_DATA" \
  --tool-directory "$BUNDLE" --receipt-directory "$RAW/before-cache-tag-record"
```

Keep the old failed `clean.json`, clean log and both failed managed receipts exactly
as recorded. `audit.py clean` intentionally refuses to resend once `clean.json`
exists; do not delete/overwrite it or call that action again. After successful
repair and unchanged raw-artifact hash verification, run the original Cargo cleanup
in another NEW exclusive managed job. The admission below is read-only code from
the unchanged original bundle; its final exec is the original `cargo clean
--manifest-path <workspace/api/Cargo.toml> --target-dir <ownership target>`:

```bash
TARGET="$DATA/target"
systemd-run --user --scope --slice=issue2824.slice --unit=issue2824-before-clean-repaired \
  python3 "$BUNDLE/resource-job-run.py" --deadline-seconds 180 \
  --output-dir "$RAW/before-clean-repaired-job" -- \
  python3 "$BUNDLE/resource-heavy-run.py" env CARGO_BUILD_JOBS=2 python3 -c \
  'import os,sys; from pathlib import Path; sys.path.insert(0,sys.argv[1]); from sampling import admission; admission(Path(sys.argv[2])); admission(Path(sys.argv[3])); os.execvp("cargo",["cargo","clean","--manifest-path",str(Path(sys.argv[2])/"api/Cargo.toml"),"--target-dir",sys.argv[3]])' \
  "$BUNDLE" "$WORKSPACE" "$TARGET"
```

Preserve the original pre-metadata measurement/volume/bin hashes, external repair
record/marker hash, unchanged raw-data hash comparison, and final Cargo command/log,
exit/cleanup/OOM receipts. Candidate cleanup uses its exact source SHA and separate
new receipt paths. This metadata-only step follows measurement; it never enters
CPU/RSS/volume comparisons or modifies the formal campaign/measurement artifacts.

After Root restores exact assembly and freezes, fresh QA uses the same `run` recipe
with `--source-sha "$CANDIDATE_SHA" --role candidate` for dev2, dev10 and test2,
new outputs and the same absolute campaign. These are the three candidate cold slots.
Keep dev2 target until the matched incremental recipe finishes:

```bash
python3 "$BUNDLE/audit.py" run --workspace "$WORKSPACE" --source-sha "$CANDIDATE_SHA" \
  --role candidate --workload incremental --reuse-target-from "$RAW/after-dev2/data" \
  --campaign "$RAW/campaign.json" --output "$RAW/after-incremental" --deadline-seconds 900
```

The cached exact-bin warmup is recorded separately, with pre-existing extra bins
explicitly labeled unused artifacts. Only the subsequent mutation phase compares
to Scout incremental. The exact get_session contextual anchor replaces its response
expression with the original equivalent local variable. Original bytes/hash/mode,
changed hash and patch are durably retained before mutation. Finally/cancellation
restores only recognized original/changed bytes; it never overwrites an independent
edit. SIGKILL or an unknown state retains recovery evidence. Inspect the original
job first, then use `audit.py recover --workspace ... --backup-directory
<after-incremental/data/recovery>`; a different source, altered backup or unrelated
handler edit fails closed for Root inspection. Verify a clean source afterward.

Then clean dev2/dev10 through the bounded wrapper above with their own new cleanup
receipts. Keep test2 target for workspace listing/full backend first-compilation
instrumentation, so newly compiled domain lib-test units also get the same wrapper:

```bash
python3 "$BUNDLE/audit.py" run --workspace "$WORKSPACE" --source-sha "$CANDIDATE_SHA" \
  --role candidate --workload external --reuse-target-from "$RAW/after-test2/data" \
  --campaign "$RAW/campaign.json" --output "$RAW/workspace-list" --deadline-seconds 2400 \
  -- cargo test --manifest-path api/Cargo.toml --locked --offline --workspace -- --list
```

Use a separate external observation for the unchanged original full Node backend
command. Configure the real final-SHA P0 metadata capture before either parent Cargo
test command; no nested Cargo refresh is allowed. Preserve jobs/threads and original
filters. Root-only `test2` is not proof of every domain's lib-test peak. Additional
external command observations are individually recorded, never averaged into cold
samples. Cleanup test2 only after all dependent observations/target-volume evidence.

## Report generation and initial observations

The committed generator writes a new JSON and Markdown artifact directory:

```bash
python3 "$BUNDLE/report.py" --workspace "$WORKSPACE" \
  --baseline-source dc1aa27f9658b2aaf17541a23c8a76db77016f53 \
  --candidate-source "$CANDIDATE_SHA" \
  --measurement "$RAW/before-dev2/data/measurement.json" \
  --measurement "$RAW/after-dev2/data/measurement.json" \
  --measurement "$RAW/after-dev10/data/measurement.json" \
  --measurement "$RAW/after-test2/data/measurement.json" \
  --measurement "$RAW/workspace-list/data/measurement.json" \
  --incremental "$RAW/after-incremental/data/incremental.json" \
  --scout-directory "$WORKSPACE/tmp/test-governance/compile-memory-audit" \
  --ci-before "$WORKSPACE/tmp/test-governance/2824-ci-baseline/ci-before-verify-metrics.json" \
  --before-inventory "$WORKSPACE/tmp/test-governance/2824-contract-baseline/runtime-test-inventory.json" \
  --after-inventory "$RAW/runtime-test-inventory.json" \
  --identity-map "$RAW/runtime-identity-remap.json" \
  --defect-evidence "$RAW/controlled-defect-evidence.json" --output "$RAW/final-report"
```

Omit unavailable inputs while preparing the initial report; they remain null. Actual
runtime inventory records are package/target/source/kind/function instances. Moved
identities require a source-bound one-to-one remap (`baseline_sha`, `candidate_sha`,
`pairs` containing full before/after records); duplicate/reused/nonexistent identities
are rejected. Keep original target multiplicity and ignored-state checks. Function
name deduplication cannot stand in for retention. Listing alone is discovery, not
executed tests or defect-detection evidence: actual controlled bad fixtures and
unchanged assertion/dependency evidence remain separate inputs for independent QA.
The report never manufactures their verdict from a passing build.

| Canonical workload | Before | Candidate | Status |
| --- | --- | --- | --- |
| ALL-bin dev2 CPU/RSS/anon/target+all-bin volume | null | null | B1, then fresh QA |
| ALL-bin dev10 | null | null | fresh QA absolute limits |
| Root test2 and additional domain lib-test PID/RSS | null | null | fresh QA; external compile observations retained |
| actual linker PID peak / conservative descendant bound | null | null | separate scopes; sampled peak limits explicit |
| matched mutation CPU/RSS/anon | historical sample below | null | fresh QA cached exact-bin recipe |
| executed defect-detection retention / multiplicity / new-ignore result | baseline runtime discovery only | null | independent final QA |

Original Scout source is protected beta. Raw artifacts remain under
`tmp/test-governance/compile-memory-audit/`, with original `scout-incremental-job`
receipts. Each phase is n=1, exact-bin only:

| Phase | Wall s | CPU user s | CPU sys s | invocation/descendant maximum RSS KiB | sampled anon B |
| --- | ---: | ---: | ---: | ---: | ---: |
| original exact-bin warmup | 467.925798 | 596.93 | 137.55 | 9290240 | 8861024256 |
| original equivalent session edit | 33.500343 | 31.31 | 10.39 | 6212612 | 5462794240 |

Historical direct linker PID data, canonical ALL-bin CPU/storage and full target/bin
volumes were not captured by Scout and remain null. Do not reinterpret Scout's
exact-bin descendant bound as a direct linker sample or canonical ALL-bin baseline.
The generator retains original per-rustc TSV rows and hashes of its input artifacts.
Baseline runtime discovery contains 7271 instances, including 12 ignored and 50
multi-target functions; candidate retention/execution remains null before QA.

CI before consists of three completed historical beta verify **failures**:
43467252825/dc1aa27f (26.75 elapsed job minutes), 43436478830/a896eaa4 (47.10),
43398609486/3276a1f5 (21.65). These are sums of actual job durations, excluding queue
and billing rounding. After samples, P50 and P95 remain null with insufficient
samples; the generator requires at least 20 completed samples of one conclusion
for descriptive quantiles. These historical failures establish no causal speedup.
Governance net diff records exact source SHAs and added/removed lines in scripts/,
.github/ and all AGENTS.md files; it is not a benefit attribution or a guard change.

Final report artifacts must include raw workload exit/cleanup/OOM, per-rustc records,
real process identities/sampling limits, one-scope CPU, source/profile/cache and
volume accounting, matched incremental restoration, actual runtime multiplicity,
controlled negatives and CI sample limits. Code/generator/fixtures are committed
before freeze; afterward attach evidence artifacts without adding functionality.
