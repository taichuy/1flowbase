# Narrow tungstenite frame-buffer ownership patches

These are two separate, complete production Cargo package sources, not a replacement
WebSocket implementation. Both retain their original manifests, features and licenses
(MIT OR Apache-2.0; see each package's LICENSE-MIT and LICENSE-APACHE).

| Local package | Original identity | Original source directory |
| --- | --- | --- |
| tungstenite-0.27.0-openai-memory | https://github.com/openai-oss-forks/tungstenite-rs revision 4fffad30fe373adbdcffab9545e9e9bf4f2fc19f, package 0.27.0 | /home/taichuy/.cargo/git/checkouts/tungstenite-rs-006aed970da0a0a9/4fffad3 |
| tungstenite-0.29.0-memory | crates.io tungstenite 0.29.0; packaged VCS metadata c2921718b76f0ed8c69a895397ed981e4a321f6f | /home/taichuy/.cargo/registry/src/rsproxy.cn-e3de039b2554c837/tungstenite-0.29.0 |

`original-sha256.json` records SHA-256 of every copied ORIGINAL file, keyed by
package and relative path. These are locally computed file hashes, not registry
package integrity checksums or upstream attestations. Original key hashes:

| Package | Cargo.toml SHA-256 | src/protocol/frame/mod.rs SHA-256 |
| --- | --- | --- |
| 0.27.0 fork | d83cc5d81bfbf9c793e261be449cb3e115894e49b3d35d5ca7e7bce3d7190817 | f109e5e835c54a10365a0af7fe1e9e830cf7193bae3565b706b99606975e2150 |
| 0.29.0 registry | 71403686d95f9b517c04a3c7d50434613bcab363303fce427b1064184e597811 | 7f1c37d4626d2fbffe0adcc0da85fe515612be9395d3aaaa93dc82edf0ded986 |

## Package boundary and reproduction

For either original source above, copy directories `src`, `tests`, `examples`,
`benches` when present; copy the root files `Cargo.toml`, `Cargo.toml.orig`,
`Cargo.lock`, `README.md`, `CHANGELOG.md`, `LICENSE-MIT`, `LICENSE-APACHE`,
`rustfmt.toml`, `.cargo_vcs_info.json` when present. This includes all existing
manifest targets and package-required files. No manifest target refers to an
external script. Do not copy `.git`, `.cargo-ok`, `target`, `.github`, fuzz or
Autobahn corpus/results. Original copied footprints are 53 files / 401409 bytes
(fork) and 39 files / 284468 bytes (registry).

To reproduce a clean snapshot precisely, read the relative file keys for each
package from `original-sha256.json`, copy those paths from its exact original
source into a new directory, and verify each file using `hashlib.sha256`.
Compare that snapshot against this vendor directory with `diff -ru`: the only
modified original file must be `src/protocol/frame/mod.rs`, and the only added
source file must be `src/protocol/frame/memory_tests.rs`. The original manifests,
existing tests, and all other implementation files must remain byte-identical.

## Changes and ownership

`FrameCodec` tracks oversized read backing before header advance/payload split
hides capacity, including initialization from partially read data and reservations.
After a complete frame, it detaches the entire unread tail into an independent
buffer only when the tail fits the existing configured read target (including
the existing protocol-header minimum). A still-large tail is retained until a
later boundary; ordinary normal backing is left alone. Held frame Bytes remain
valid on their original allocation. Partial headers/bodies and WouldBlock state
remain owned by the codec until complete.

After every queued output byte has successfully reached Write, the empty output
Vec is replaced only when capacity exceeds the existing write target. A zero
target releases backing. Partial writes/errors retain the exact remainder;
transport flush stays a separate caller operation. No deflate state is reset.
No mask, role, UTF-8, fragmentation, handshake, compression, message/frame limits,
TTL, drop policy or dependency version was changed.

The focused tests cover hidden backing, partially read large backing and tails,
held payload validity, ordinary small frames, partial extended header/body across
WouldBlock, FIFO, complete writes, partial writes/errors/retries, zero writes and
zero/nonzero write targets. They have been written but not compiled or executed
by the developer; Root owns the centralized test batch and independent protocol
and allocator fixtures.

## Dependency resolution and removal

`api/Cargo.toml` patches only the original OpenAI tungstenite git source to the
0.27.0 vendor package and crates.io tungstenite 0.29.0 to the other package;
both directories are excluded from workspace membership. The paired tokio fork
revision 0e5b2d73aa18dd9f0a50ee9ff199d5aef7594186 and the 0.30 development
dependency remain unchanged. Workspace patches do not propagate to standalone
fixture roots: Root must patch those manifests separately.

Cargo.lock has intentionally not been edited. Root must resolve it normally and
check that only the intended source substitutions occur before accepting this
candidate. No upstream acceptance is claimed. Remove the vendor patches when a
reviewed upstream version/fork provides the same ownership invariants and passes
the retained protocol/compression and allocator regressions. This change does
not establish bounds for deflate/TLS/allocator/slab or total process RSS.
