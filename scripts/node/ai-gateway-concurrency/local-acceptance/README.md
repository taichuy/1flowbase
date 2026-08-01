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

After the owned API is healthy, the runner uses owner username/password values
from named environment variables to open a process-local temporary session via
the repository-owned auth helper. It attaches that session to the owner client,
reuses the named application API key, and assigns the manifest-pinned baseline
DeepSeek installation through repository-owned plugin actions. It then selects
the pinned existing-local after installation, or uploads the local package only
when `after_installation_id` is omitted. Both paths verify the after-package
digest, preserve the publication id, and report zero network installs. The
successful artifact records `baseline_setup` and identifies `transition_mode` as
`existing_local` or `uploaded_local`. The final after installation remains
assigned for manual testing; cleanup does not roll it back. The session is revoked
before the owned API stops, with revocation failures recorded independently from
the primary result. The runner never builds, checks out, publishes, or downloads
anything.

Copy `count-tokens-upgrade.run.example.json`, replace its source receipt, binary
paths/digests, and safe local paths, then export the five named environment
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
key, owner password, temporary cookie, CSRF token, database credentials,
provider master key, or raw conversation text.

The runner derives the exact `api/apps/api-server/.env` path from the validated
api-server cwd and parses it with the repository-owned dotenv parser. That full
map seeds only the owned api-server child; it is not shell-sourced or passed to
the plugin-runner. Runner-owned development mode, port, database, plugin-runner
URL, and cookie settings override the file. A manifest may optionally name
`provider_secret_master_key` or `provider_install_root` environment overrides;
a non-empty override wins last, while missing names or values are not
configuration failures. Accidental provider settings inherited from the parent
process are excluded unless the source `.env` or an optional override supplies
them. Artifacts record only the derived `.env` path and, when the file exists,
its SHA-256; they never record parsed values. A child that exits before health
produces bounded typed stdout/stderr diagnostics, which pass through the same
secret redaction as the rest of the artifact. Dotenv diagnostic redaction is
key-aware: password, secret, token, private credential, API-key, and
credential-bearing database URL values are protected across raw and encoded
representations, while public trusted keys, schema identifiers, and paths remain
unchanged.
