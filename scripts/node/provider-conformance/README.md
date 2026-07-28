# Actual Provider Package Conformance

`cli.js` is the single owner of the paired main/official SHA artifact used to
prove actual-provider compatibility and package provenance. It accepts only
explicit source SHAs and actual `.1flowbasepkg` artifacts; it does not build a
substitute provider binary or infer behavior from source text.

The fixture fixes the matrix to OpenAI, Anthropic, Aliyun Bailian, DeepSeek,
Gemini, and OpenAI Compatible. For each unpacked package the runner checks the
current manifest contract and then loads it through the real `plugin-runner`.
The provider binary is invoked against a loopback fake upstream which verifies
the exact vendor method, path, selected headers, and JSON request body.

```bash
node scripts/node/provider-conformance/cli.js \
  --main-root "$PWD" \
  --official-root ../1flowbase-official-plugins \
  --main-sha <full-main-sha> \
  --official-sha <full-official-sha> \
  --package-dir tmp/provider-conformance/packages \
  --plugin-runner-bin api/target/release/plugin-runner \
  --fixture scripts/node/provider-conformance/fixtures/six-provider-matrix.json \
  --artifact tmp/provider-conformance/paired-sha.json
```

For every provider, the artifact records one receipt chain from the exact
official source SHA and source manifest identity/fingerprint, through the actual
package digest and reconciled installed-manifest fingerprint, to the identity
returned by the package loaded in `plugin-runner`. The runner rejects a dirty
checkout, either source SHA mismatch, an incomplete or duplicated package
matrix, any source/package/installed/runtime identity mismatch, and a package or
provider-receipt mismatch when an existing paired artifact is supplied with
`--expected-pair-artifact`.

The fixture also covers controlled negatives: a stale installed-manifest
fingerprint must be rejected by artifact reconcile before package load, legacy
Generate input must not reach a provider, and an undeclared semantic capability
must be rejected before the provider is spawned or the fake upstream is
contacted. WireAudit assertions check only bounded fields and assert that
generated prompt, system prompt, header, end-user, and secret canaries never
appear in runner output or in the artifact. Fake-upstream raw request data is
retained in memory only for the single assertion and is neither logged nor
written to disk.

The runner is intentionally executed only by the frozen Root Test Batch or its
explicit CI entrypoints. Do not use it as a package-publishing or signing step.
