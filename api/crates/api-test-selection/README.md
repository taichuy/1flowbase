# API test selection

Internal, unpublished, standard-library-only proc-macro crate. Only api-server's
dev-dependency may consume it. It neither discovers tests nor owns runtime code.

Use `#[cfg_attr(test, api_test_selection::group(core, contracts))]` on an
original test-only item or a proved test-only descendant. Keep the original
`cfg(test)` guard. Labels map exactly to Cargo's crate names:

| Owner | CARGO_CRATE_NAME |
| --- | --- |
| core | api_server |
| contracts | api_private_contracts |
| public_api | api_public_api |
| routes | api_private_routes |

Owners are a nonempty identifier list separated by single commas, without a
trailing comma. Unknown/duplicate labels, malformed tokens and missing/unknown
crate names produce compile_error. Both attributes validate before selection.
`group` returns the original item for an owner and no item otherwise.
`fixture` always returns the original item; only a non-owner gets an item-level
`allow(dead_code)`. Production builds do not expand a test-only cfg_attr.

The four standard Cargo examples are test harnesses with the exact four crate
names. Shared fixtures cover all 15 nonempty owner subsets. Each root's ungated
invariant calls its eight expected cases, so deleting selected cases cannot give
an empty green suite; retained non-owner cases fail their handwritten membership
assertions. External/inline modules retain private super bindings. Multi-owner
helpers and always-retained fixture types are checked under deny(dead_code).
The owner reads its fixture fields; non-owner fields remain unused and need the
local allow. Root's compile-negative checks should additionally verify malformed
attributes, missing/unknown CARGO_CRATE_NAME and owner dead-code diagnostics.

Planned central commands (not executed during implementation):

```text
cargo test --manifest-path api/Cargo.toml -p api-test-selection --examples --locked
cargo test --manifest-path api/Cargo.toml -p control-plane-postgres-tests --test dependency_boundaries --locked
```

No test starts Cargo or rustc recursively in this crate. Workspace metadata and
dependency-boundary execution belong to the central validation batch.
