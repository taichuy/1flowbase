# Local AI Gateway acceptance

The existing harness owns both the full local client matrix and the executable
CountTokens upgrade scenario. The upgrade runner verifies the gate-produced
main-source receipt, verifies `main_source_root` has that exact Git HEAD, requires
`api_server_cwd` to be its `api/apps/api-server` directory, and verifies the
expected SHA-256/source SHA of the frozen api-server and plugin-runner binaries.
It then starts both binaries as owned processes on
ephemeral loopback ports. It connects them to the configured development database
only to reuse the fixed application/publication data. Shared API processes are
never proxied, reused, stopped, or restarted.

The runner reuses an existing owner session and application API key from named
environment variables, and uploads a local DeepSeek package through the
repository-owned plugin install action. It never builds, checks out, publishes,
or downloads anything.

Copy `count-tokens-upgrade.run.example.json`, replace its source receipt, binary
paths/digests, and safe local paths, then export the seven named environment
variables. Run exactly:

```bash
node scripts/node/ai-gateway-concurrency/local-acceptance/cli.js \
  count-tokens-upgrade --manifest /absolute/path/to/count-tokens-upgrade.run.json
```

The application id is fixed to `019f5443-5b8e-74b2-90e3-c867dbddd37b`.
Owned service ports are always ephemeral; `3100`, `7800`, and `7801` are
rejected. The artifact is written beneath
`tmp/test-governance` and contains installation checksums/versions, the unchanged
publication id, frozen binary paths/digests/source SHA/ports, CountTokens result,
hashed conversation summaries, provenance,
and independent primary/cleanup failures. It never contains the application
key, owner cookie, CSRF token, or raw conversation text.

The api-server child runs from the validated source cwd so its normal development
`.env` loading remains available; explicit manifest-selected environment values
still override its port, database, plugin-runner URL, provider master key, and
provider install root. A child that exits before health produces bounded typed
stdout/stderr diagnostics, which pass through the same secret redaction as the
rest of the artifact.
