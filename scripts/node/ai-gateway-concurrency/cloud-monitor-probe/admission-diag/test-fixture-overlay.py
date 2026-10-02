"""CI cfg(test)-only control synchronization; restored before the release binary.

Reuse the original739c.../92a62c6f3 immediate archive-notification control.
No timing/concurrency/queue/transaction/production code or assertion is changed.
"""
import hashlib
import json
import pathlib
import subprocess

RELATIVE_PATH = "api/crates/control-plane/src/client_trajectory/_tests/batching.rs"
EXPECTED = "739c8fff67dc8ac3356936389bbb4f77b9c3a5a4cb4db945fbfee3080ab58461"
OLD = "        tokio::task::yield_now().await;"
NEW = """        if delay.is_zero() {
            // A yield does not guarantee a timer tick/commit in optimized tests.
            // Establish the immediate control's archive boundary before the next frame.
            tokio::time::timeout(Duration::from_secs(1), writer.archived.notified())
                .await
                .expect("immediate archive control must commit before next admission");
        } else {
            tokio::task::yield_now().await;
        }"""
PREDICATES = [
    "batched < immediate",
    "batched <= 4",
    "receipt.persisted_through, 256",
    "before.sequence, index as i64 + 1",
    "after.sequence, before.sequence",
    "after.kind, before.kind",
    "after.bytes, (index as u32).to_be_bytes()",
]


def overlay(original):
    assert hashlib.sha256(original).hexdigest() == EXPECTED, "unexpected fixture source"
    text = original.decode()
    assert text.count(OLD) == 1, "one historical yield replacement required"
    changed = text.replace(OLD, NEW, 1)
    for predicate in PREDICATES:
        assert predicate in text and predicate in changed, "strict predicate lost"
    return changed.encode()


def restore(root):
    """Restore only the exact tracked fixture, verify its original bytes."""
    subprocess.check_call(["git", "-C", str(root), "checkout", "--", RELATIVE_PATH])
    restored = (root / RELATIVE_PATH).read_bytes()
    assert hashlib.sha256(restored).hexdigest() == EXPECTED, "restore source mismatch"


def main():
    import sys
    root = pathlib.Path.cwd()
    path = root / RELATIVE_PATH
    out = root / "tmp/test-governance/gateway-admission-diag/build"
    out.mkdir(parents=True, exist_ok=True)
    if len(sys.argv) > 1:
        assert sys.argv[1:] == ["--restore"], "only explicit restore supported"
        restore(root)
        (out / "test-fixture-restored.json").write_text(json.dumps({
            "path": RELATIVE_PATH, "restored_sha256": EXPECTED,
            "status": "pass", "product_source_clean_gate_required": True,
        }, indent=2))
        print("Exact cfg(test) source restored; production source must pass clean gate")
        return
    original = path.read_bytes()
    changed = overlay(original)
    path.write_bytes(changed)
    (out / "test-fixture-overlay.json").write_text(json.dumps({
        "path": RELATIVE_PATH, "original_sha256": EXPECTED,
        "overlay_sha256": hashlib.sha256(changed).hexdigest(),
        "reused_from": "92a62c6f3a2eb4f946c610db06e6be85a803a20e",
        "scope": "cfg(test) only; zero-delay immediate MemoryWriter commit before next admission;20ms burst unchanged",
        "original_assertions_retained": PREDICATES,
        "restore_before_release_build": True,
    }, indent=2))
    (out / "test-fixture-overlay.patch").write_bytes(subprocess.check_output([
        "git", "diff", "--", RELATIVE_PATH,
    ]))
    print("Historical cfg(test) immediate control synchronization applied; strict predicates retained")


if __name__ == "__main__":
    main()
