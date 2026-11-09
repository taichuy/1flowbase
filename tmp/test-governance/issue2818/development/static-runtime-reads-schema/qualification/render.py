#!/usr/bin/env python3
"""Render only; compilation/execution and matching cache flags belong to CI owner."""
import argparse, hashlib, json, re, subprocess
from pathlib import Path
SOURCE_SHA = "903971d5c3ddd37f93c1f7314e02dae4a2bb120c"
INPUT_SHA = "7bb249e47d29dce87a291677884bd42962aa2046"
HERE = Path(__file__).resolve().parent
SOURCE = "api/apps/api-server/src/routes/applications/application_runtime/interface_runtime_reads/managed_projection/application_runtime_reads_output.rs"
SCHEMA = str(Path(SOURCE).with_name("application_runtime_reads_schema.rs"))
MP = "api/apps/api-server/src/extension_bus/managed_projection.rs"
def sha(data): return hashlib.sha256(data).hexdigest()
def function(data):
    text = data.decode(); masked = re.sub(r'//[^\n]*|/\*.*?\*/|"(?:\\.|[^"\\])*"', lambda m: " "*len(m[0]), text, flags=re.S)
    match = re.search(r"\bfn\s+managed_projection_schema\b", masked)
    start = match.start(); end = masked.index("{", match.end()) + 1; depth = 1
    while depth: depth += (masked[end] == "{") - (masked[end] == "}"); end += 1
    return text[start:end].encode(), text[end:].encode()
def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--serde-rlib", type=Path)
    args = parser.parse_args(); root = args.repo_root.resolve(); output = args.output.resolve()
    mapping = json.loads((HERE.parent/"source-map.json").read_text())
    git = lambda revision, path: subprocess.check_output(["git", "show", revision+":"+path], cwd=root)
    for path in [SOURCE, SCHEMA, MP]:
        assert (root/path).read_bytes() == git(SOURCE_SHA, path), ("workspace is not fixed candidate", path)
    old_file = git(INPUT_SHA, SOURCE); new_file = (root/SOURCE).read_bytes()
    old_body, old_suffix = function(old_file); new_body, new_suffix = function(new_file)
    assert old_body == (HERE.parent/"original-function.rs").read_bytes()
    assert sha(old_body) == mapping["original_function"]["sha256"]
    assert sha(new_body) == mapping["new_function"]["sha256"]
    assert old_suffix == new_suffix and sha(new_suffix) == mapping["protected_suffix"]["sha256"]
    assert sha((root/MP).read_bytes()) == mapping["helper_dependency"]["sha256"]
    schema_bytes = (root/SCHEMA).read_bytes()
    assert sha(schema_bytes) == mapping["files"][SCHEMA]["after_sha256"]
    assert not re.search(rb"OnceLock|LazyLock|from_str|from_slice|\.clone\(|\.cloned\(|Box::|Mutex|RwLock", schema_bytes)
    template = (HERE/"host.rs.in").read_text()
    substitutions = {"MP_PATH": json.dumps(str(root/MP), ensure_ascii=False), "SCHEMA_PATH": json.dumps(str(root/SCHEMA), ensure_ascii=False),
                     "OLD_FUNCTION": old_body.decode(), "NEW_FUNCTION": new_body.decode(), "SOURCE_SHA": SOURCE_SHA}
    rendered = template
    for key, value in substitutions.items(): rendered = rendered.replace("@@"+key+"@@", value)
    assert "@@" not in rendered
    output.parent.mkdir(parents=True, exist_ok=True); output.write_text(rendered)
    rlib = args.serde_rlib.resolve() if args.serde_rlib else None
    manifest = {"source_sha": SOURCE_SHA, "original_input_sha": INPUT_SHA, "source_root": str(root),
        "source_binding": {path: sha((root/path).read_bytes()) for path in [SOURCE, SCHEMA, MP]},
        "old_function_sha256": sha(old_body), "new_function_sha256": sha(new_body), "protected_suffix_sha256": sha(new_suffix),
        "template_sha256": sha((HERE/"host.rs.in").read_bytes()), "renderer_sha256": sha(Path(__file__).read_bytes()),
        "rendered_host": str(output), "rendered_host_sha256": sha(output.read_bytes()),
        "serde_json_rlib": {"path": str(rlib), "sha256": sha(rlib.read_bytes())} if rlib else "CI owner must select exact API cache artifact",
        "owner_parameters_pending": ["rustc toolchain/version", "exact serde_json API artifact/features/dependency search path", "matching profile/optimization flags", "fixed measured/warmup iterations"],
        "measurement": {"noise_control": "same normal binary with W6_OLD_CONTROL=1 binds both sides to old::schema with old_control_a/b labels; 5 raw alternating pairs", "fixed_timing_arguments": "1000 iterations, 100 equal warmups; CI owner may confirm within 120s before any outcomes", "fixed_instrumented_arguments": "1 iteration per side, 100 equal warmups; separate binary", "decision": "Root predeclares maximum old-old pair relative difference as conservative noise band; in-band old/new difference is unresolved; no post-result sample tuning or retry for PASS", "samples": "five fixed alternating pairs", "timed_region": "construction and drop only", "instrumentation": "separate binary compiled with --cfg heap_counts; do not use its timings", "requested_peak": "successful GlobalAlloc boundaries; System realloc internal transient/usable-size/RSS is unknown", "stack_peak": "unknown", "rodata_delta": "unknown; combined host total is not a delta"},
        "status": "rendered and source-bound only; no Rust compile/runtime outcome"}
    target = output.with_suffix(".manifest.json"); target.write_text(json.dumps(manifest, indent=2)+"\n")
    print(str(target))
if __name__ == "__main__": main()
