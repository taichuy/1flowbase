# Local AI Gateway acceptance

The existing harness owns both the full local client matrix and the executable
CountTokens upgrade scenario. The upgrade runner connects only to explicitly
configured loopback endpoints, reuses an existing owner session and application
API key from named environment variables, and uploads a local DeepSeek package
through the repository-owned plugin install action. It never publishes an
application or downloads a package.

Copy `count-tokens-upgrade.run.example.json`, replace its safe paths and isolated
loopback origins, then export the four named environment variables. Run exactly:

```bash
node scripts/node/ai-gateway-concurrency/local-acceptance/cli.js \
  count-tokens-upgrade --manifest /absolute/path/to/count-tokens-upgrade.run.json
```

The application id is fixed to `019f5443-5b8e-74b2-90e3-c867dbddd37b`.
Ports `3100`, `7800`, and `7801` are rejected. The artifact is written beneath
`tmp/test-governance` and contains installation checksums/versions, the unchanged
publication id, CountTokens result, hashed conversation summaries, provenance,
and independent primary/cleanup failures. It never contains the application
key, owner cookie, CSRF token, or raw conversation text.
