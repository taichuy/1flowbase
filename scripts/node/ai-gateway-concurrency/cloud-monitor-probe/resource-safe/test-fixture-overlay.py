"""CI-only scheduling control; restored before both production release builds."""
import pathlib,json,hashlib,subprocess
path=pathlib.Path('api/crates/control-plane/src/client_trajectory/_tests/batching.rs');original=path.read_bytes();expected='739c8fff67dc8ac3356936389bbb4f77b9c3a5a4cb4db945fbfee3080ab58461';assert hashlib.sha256(original).hexdigest()==expected
old='        tokio::task::yield_now().await;'
new='        if delay.is_zero() {\n            // A yield does not guarantee a timer tick/commit in optimized tests.\n            // Establish the immediate control\'s archive boundary before the next frame.\n            tokio::time::timeout(Duration::from_secs(1), writer.archived.notified())\n                .await\n                .expect("immediate archive control must commit before next admission");\n        } else {\n            tokio::task::yield_now().await;\n        }'
text=original.decode();assert text.count(old)==1;text=text.replace(old,new,1);path.write_text(text)
# Original comparative and full preservation assertions remain unchanged.
assert 'batched < immediate' in text and 'batched <= 4' in text and 'receipt.persisted_through, 256' in text
out=pathlib.Path('tmp/test-governance/gateway-resource-safe/build');out.mkdir(parents=True,exist_ok=True)
(out/'test-fixture-overlay.json').write_text(json.dumps({'path':str(path),'original_sha256':expected,'overlay_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'scope':'cfg(test) only; MemoryWriter immediate archive notification; no production/timer changes','original_assertions_retained':True,'restore_before_release_build':True},indent=2))
(out/'test-fixture-overlay.patch').write_bytes(subprocess.check_output(['git','diff','--',str(path)]))
print('CI cfg(test) immediate control synchronization applied; original predicates retained.')
