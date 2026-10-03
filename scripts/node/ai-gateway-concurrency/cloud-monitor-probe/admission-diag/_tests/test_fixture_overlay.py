"""Pure Python/source/filesystem tests: no Rust/compiler/service/network."""
import hashlib
import importlib.util
import pathlib
import subprocess
import tempfile
import unittest

MODULE = pathlib.Path(__file__).resolve().parents[1] / "test-fixture-overlay.py"
spec = importlib.util.spec_from_file_location("fixture_overlay", MODULE)
fixture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture)


class OverlayTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # The same immutable public source already used by every gate.
        cls.original = subprocess.check_output([
            "git", "show", "5e3afcefd5cf6c3e09b345496b5956e30426cc40:" + fixture.RELATIVE_PATH,
        ])

    def test_exact_historical_source_and_strict_assertions(self):
        changed = fixture.overlay(self.original)
        self.assertEqual(hashlib.sha256(self.original).hexdigest(), fixture.EXPECTED)
        for predicate in fixture.PREDICATES:
            self.assertIn(predicate.encode(), changed)
        self.assertEqual(changed.decode().count("writer.archived.notified()"), 2)
        self.assertIn(b"if delay.is_zero()", changed)
        self.assertIn(b"} else {\n            tokio::task::yield_now().await;", changed)

    def test_foreign_already_patched_or_missing_predicate_source_refused(self):
        for changed in [self.original + b"\n", fixture.overlay(self.original),
                        self.original.replace(b"batched < immediate", b"true")]:
            with self.assertRaises(AssertionError):
                fixture.overlay(changed)

    def test_restore_only_expected_tracked_fixture_on_success_or_failed_gate(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            subprocess.check_call(["git", "init", "-q", str(root)])
            path = root / fixture.RELATIVE_PATH
            path.parent.mkdir(parents=True)
            path.write_bytes(self.original)
            subprocess.check_call(["git", "-C", str(root), "add", fixture.RELATIVE_PATH])
            subprocess.check_call(["git", "-C", str(root), "-c", "user.name=fixture",
                                   "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture"])
            other = root / "other.txt"
            other.write_text("unrelated must remain")
            for simulated_gate_failure in [False, True]:
                path.write_bytes(fixture.overlay(self.original))
                try:
                    if simulated_gate_failure:
                        raise RuntimeError("simulated gate failure")
                except RuntimeError:
                    pass
                finally:
                    fixture.restore(root)
                self.assertEqual(path.read_bytes(), self.original)
                self.assertEqual(other.read_text(), "unrelated must remain")

    def test_actual_exit_trap_preserves_gate_exit_or_fails_closed_on_restore_error(self):
        gate = (MODULE.parent / "ci-gates.sh").read_text()
        start = gate.index("restore_test_fixture() {\n")
        end = gate.index("\n}\n", start) + 3
        function = gate[start:end]
        trap = next(line for line in gate.splitlines() if line.startswith("trap "))
        # Only this local child-shell test stubs the restoration interpreter;
        # no Rust, credentials, network, product files or global settings touched.
        for primary_exit, restore_exit, expected in [(0, 0, 0), (17, 0, 17),
                                                     (0, 23, 99), (17, 23, 99)]:
            script = "\n".join([
                "set -euo pipefail", "overlay_active=1", "controls=/unused",
                function,
                f"python3() {{ printf 'restore_command_exit={restore_exit}\\n'; return {restore_exit}; }}",
                trap, f"printf 'original_gate_exit={primary_exit}\\n'", f"exit {primary_exit}",
            ])
            result = subprocess.run(["bash", "-c", script], text=True, capture_output=True)
            self.assertEqual(result.returncode, expected, result.stdout + result.stderr)
            self.assertIn(f"original_gate_exit={primary_exit}", result.stdout)
            self.assertIn(f"restore_command_exit={restore_exit}", result.stdout)


if __name__ == "__main__":
    unittest.main()
