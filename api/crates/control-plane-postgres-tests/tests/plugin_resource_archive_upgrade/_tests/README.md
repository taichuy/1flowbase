# Root #2896 — private archive acceptance fixture (implementation, not QA)

## Evidence boundary

Two **ignored-by-default** cases in `plugin_resource_archive_upgrade`:

1. `ssh_archives_explicit_preparation_preserves_full_original_state`
   - Uses actual `PluginManagementService::install_uploaded_plugin` twice, not prepared
     installation inputs, catalog reconstruction, directory installs or relabelled binaries.
   - Requires the authoritative **0.1.21** archive with SHA-256
     `7f91e97b29ccdff563ead49d46f41864061424bf43e164785047868840b9ebc2`.
   - Requires an actual **0.1.25** package produced from BUSINESS BUILD commit
     `8e0ada94257c256809d47bf2684130f2c5cc7c51`, with its approved source manifest and
     separately supplied freshly built executable. Compares archive checksum, installed
     identities, exact source-manifest bytes and executable hashes; rejects the old executable.
   - Calls the production `ManagedPluginActivationService::prepare` explicitly with a real
     repository-loaded Root actor. Old/full and new/full schemas come from those archives;
     every field of every historical owned collection receives a local synthetic row.
   - Verifies historical physical table OIDs/names, columns/types/defaults, complete rows,
     owner ledger identity, original version membership, credential ciphertext and decryptability,
     original record ACL/policy/grants/revisions, old settings defaults and contribution grants.
   - Seeds data/credentials through durable ports and ACLs through `RecordPermissionService`.
     Includes real member role admission, ungranted-action/operation/caller-workspace negatives,
     cross-owner/scope credential isolation, no-upload-authority, idempotent approvals and
     nonroot inability to self-approve or revive revoked authority.
2. `monitor_archive_enable_rejects_missing_and_disabled_provider_without_authority`
   - Separately requires the actual BUSINESS **iqlater/compute-monitor@0.2.7** archive and
     fresh executable; absence of inputs is an error, never a skip inside a passing case.
   - Actual `enable_plugin` commands reject missing/disabled `taichuy/ssh` with the exact
     structured contract error and dependent identity, without granting authority or
     activating the consumer. Consumer has no duplicate schema.

**These are service/storage integration assets, not HTTP/runtime results.** No lifecycle
repository write manufactures `ActiveRequested`, worker readiness, registry publication or
API generation. `RuntimeUnavailable` is a fail-closed, unused runtime boundary, not a runtime
simulator: a call into it errors. Both installed artifacts must remain Disabled/Inactive even
after successful preparation. Schema/authority preparation is **not successful enable**.

**U1 is only advanced through genuine package intake + full schema/authority preparation;
it is not complete running old-to-new version-family migration evidence. U4 stays UNVERIFIED.**
The precise missing composition is the real control-plane `ProviderRuntimePort` bound by the
API host to its `RuntimeExtensionHost`, production managed registration/generation publication,
and caller/provider callback adapters. This crate has no such existing assembly/dependency.
Do not add a second application composition root, successful stub activation, fake registry or
new dependency exception to close that gap. Root owns actual API propagation/activation evidence.
Enabled consumer reuse, missing-export-with-active-provider, provider disable/uninstall/switch
protection, live revocation and hot-generation assertions are deliberately not simulated here.

## Private artifact prerequisites

All credentials, commercial packages, extracted payloads and generated rows stay in an
explicit owner-private local temp workspace and a disposable `PostgresTestSchema` database.
No real SSH connection is attempted; runtime binaries are **not executed** by these cases.

- Linux x86_64 GNU fixture host, `sha256sum`, `git`, installed Rust toolchain/dependencies.
- Root's successful **fresh BUSINESS compile receipt** for each supplied executable. Hashes
  establish byte identity, not independent proof of compilation provenance. Do not point at
  stale HOST `api/target/debug` binaries from November 17/19. This worker does not build them.
- Explicit `DATABASE_URL` to a **local loopback disposable-test PostgreSQL** account with
  CREATEDB/migration privileges. There is no credential-bearing fallback or service startup.
  `PostgresTestSchema` owns a new temporary database + schema; formal migrations run there.
- `TMPDIR` must exist, be absolute, owner-private (0700), and outside HOST/BUSINESS. Production
  archive intake also uses it. Drop closes the pool-owned schema guard; fixture files are removed.
  Root should remove the entire private workspace on completion, including intake temp files.
- BUSINESS must be the approved HEAD with no tracked SSH/monitor candidate changes.
- `PLUGIN_SHARED_RESOURCE_CONTRACTS_JSON` must be **absent**, not an empty allowlist.
- For every explicitly selected case, all of its input variables below are mandatory.

## Root preparation (do not execute concurrently with another heavy command)

Use Root's already allocated heavy slot/budget. Never build/install dependencies just for this
worker. Root builds genuine provider/monitor executables first, recording commands, source
state and successful exit. Package using the existing production CLI, without signing or publishing:

```bash
HOST=/home/taichuy/git/git_worktree/lean-release-066
BUSINESS=/home/taichuy/git/git_worktree/plugin-resource-business
umask 077
PRIVATE=$(mktemp -d /tmp/root2896-archive-acceptance.XXXXXX)
mkdir -m 700 "$PRIVATE/intake" "$PRIVATE/packages"
export TMPDIR="$PRIVATE/intake"
# Root supplies an absolute FRESH_SSH_RUNTIME from its successful new BUSINESS build.
: "${FRESH_SSH_RUNTIME:?fresh BUSINESS build required}"
node "$HOST/scripts/node/plugin/cli.js" package \
  "$BUSINESS/runtime-extensions/@taichuy/ssh" --out "$PRIVATE/packages" \
  --runtime-binary "$FRESH_SSH_RUNTIME" --target x86_64-unknown-linux-gnu
```

For the separate monitor case, use the same CLI command with
`$BUSINESS/runtime-extensions/@IQLater/compute-monitor` and its independently fresh
`$FRESH_MONITOR_RUNTIME`. Choose the **actual output filenames** reported by the packager;
do not rename old binaries, fabricate release metadata, or modify package manifests.

Set provider inputs (hash variables are bare lowercase hex, without `sha256:`):

```bash
export PLUGIN_RESOURCE_BUSINESS_ROOT="$BUSINESS"
export PLUGIN_RESOURCE_OLD_PACKAGE=/home/taichuy/git/1flowbase-plugins-business/tmp/test-governance/ssh-release-0.1.21/packages/Taichuy@ssh@0.1.21@linux-amd64.1flowbasepkg
# Set this to the actual new SSH package emitted above, not a hypothetical release URL:
: "${NEW_SSH_PACKAGE:?actual freshly packaged SSH archive path required}"
export PLUGIN_RESOURCE_NEW_PACKAGE="$NEW_SSH_PACKAGE"
export PLUGIN_RESOURCE_NEW_PACKAGE_SHA256=$(sha256sum "$NEW_SSH_PACKAGE" | cut -d ' ' -f 1)
export PLUGIN_RESOURCE_FRESH_RUNTIME="$FRESH_SSH_RUNTIME"
export PLUGIN_RESOURCE_FRESH_RUNTIME_SHA256=$(sha256sum "$FRESH_SSH_RUNTIME" | cut -d ' ' -f 1)
: "${DATABASE_URL:?explicit local test PostgreSQL URL required}"
export DATABASE_URL
```

For the monitor case, additionally set:

```bash
: "${NEW_MONITOR_PACKAGE:?actual freshly packaged monitor archive path required}"
: "${FRESH_MONITOR_RUNTIME:?fresh BUSINESS monitor build required}"
export PLUGIN_RESOURCE_CONSUMER_PACKAGE="$NEW_MONITOR_PACKAGE"
export PLUGIN_RESOURCE_CONSUMER_PACKAGE_SHA256=$(sha256sum "$NEW_MONITOR_PACKAGE" | cut -d ' ' -f 1)
export PLUGIN_RESOURCE_FRESH_MONITOR_RUNTIME="$FRESH_MONITOR_RUNTIME"
export PLUGIN_RESOURCE_FRESH_MONITOR_RUNTIME_SHA256=$(sha256sum "$FRESH_MONITOR_RUNTIME" | cut -d ' ' -f 1)
```

## Exact compile / execution selectors (Root only)

Run inside Root's existing resource-budget wrapper, one Cargo at a time. Resolve jobs using the
existing `scripts/node/testing/verify-runtime.js cargo-jobs` helper before a heavy invocation.
Capture complete output and the original exit under
`$HOST/tmp/test-governance/plugin-resources/acceptance-upgrade/`; do not pipe Cargo into `head`.
Do not log database credentials, package bytes, commercial source or plaintext secrets.

Compilation only (does not prove any assertion passed):

```bash
env -u PLUGIN_SHARED_RESOURCE_CONTRACTS_JSON cargo test --offline \
  --manifest-path /home/taichuy/git/git_worktree/lean-release-066/api/Cargo.toml \
  -p control-plane-postgres-tests --test plugin_resource_archive_upgrade --no-run
```

Execute the exact provider case (must report **1 executed**, not zero):

```bash
env -u PLUGIN_SHARED_RESOURCE_CONTRACTS_JSON cargo test --offline \
  --manifest-path /home/taichuy/git/git_worktree/lean-release-066/api/Cargo.toml \
  -p control-plane-postgres-tests --test plugin_resource_archive_upgrade -- \
  --ignored --exact ssh_archives_explicit_preparation_preserves_full_original_state --test-threads=1
```

Execute the separate consumer-negative case (must report **1 executed**):

```bash
env -u PLUGIN_SHARED_RESOURCE_CONTRACTS_JSON cargo test --offline \
  --manifest-path /home/taichuy/git/git_worktree/lean-release-066/api/Cargo.toml \
  -p control-plane-postgres-tests --test plugin_resource_archive_upgrade -- \
  --ignored --exact monitor_archive_enable_rejects_missing_and_disabled_provider_without_authority --test-threads=1
```

With all inputs present, `-- --ignored --test-threads=1` executes **both** cases. A default test
run only discovering ignored cases, rustfmt, target compilation or a zero-case exit is not
acceptance evidence. No Cargo, database test, binary build or package command was run by the
fixture implementation worker. Root/fresh central QA must compile and execute it.

### Test-only descendant commits

The checkout may advance after the recorded production BUILD commit only when both plugins’ tracked `src`, Cargo metadata, manifests and UI inputs are unchanged and contain no untracked production inputs. The fixture checks ancestry and those paths; it does not relabel an older binary as compiled from the new HEAD. The original private BUILD receipt and binary/archive hashes remain required.
