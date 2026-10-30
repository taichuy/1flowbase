This isolated test host compiles the production transport lifecycle owner, its existing fixtures, and the production native error projection without linking `api-server --lib`.

From the repository root, run one Cargo command at a time through the local resource wrapper:

```sh
CARGO_TARGET_DIR="$PWD/api/target" cargo test --offline --locked \
  --manifest-path api/apps/api-server/tests/transport-session-lifecycle/Cargo.toml \
  -- --test-threads=1
```

`build.rs` selects verbatim native DTO/response items and compiles the entire production `response_projection.rs`; marker changes fail compilation. Faults use the existing control-plane test repository. The response tests exercise real services, production status projection and Axum response serialization. They do not mount the full interface registry, authenticate a live database, or exercise the production router.

The transport module and HTTP delivery module are compiled directly by path. One neighboring fixture uses a small parent adapter for preserving a primary invocation error; this adapter is not evidence for the production parent's implementation. No new production dependency or live process is required. The lockfile pins only packages already present in `api/Cargo.lock`.
