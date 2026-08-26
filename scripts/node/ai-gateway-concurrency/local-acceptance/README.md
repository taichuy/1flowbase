# Local AI Gateway acceptance

The local acceptance harness starts one owned `api-server` process backed by an
ephemeral PostgreSQL database and controlled mock upstream. Runtime extensions
are loaded by the in-process `RuntimeExtensionHost`; no second Backend process,
runtime HTTP port, or shared service is used.

Seal the current release binary and run the frozen manifest:

```bash
node scripts/node/ai-gateway-concurrency/local-acceptance/prepare-manifest.js \
  --source scripts/node/ai-gateway-concurrency/local-acceptance/manifest.json \
  --output tmp/test-governance/local-ai-gateway-manifest.json

node scripts/node/ai-gateway-concurrency/local-acceptance/cli.js run \
  --manifest tmp/test-governance/local-ai-gateway-manifest.json
```

The harness verifies pinned source and artifact receipts, protocol behavior,
durable convergence and local client results. It owns and cleans only its
temporary database, `api-server`, mock upstream, tmux sessions and evidence.
Secrets are redacted from `tmp/test-governance` artifacts.
