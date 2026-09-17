# Provider timing A/B summarizer

This entry summarizes already-sanitized direct/provider and gateway timing samples. It never opens a network connection and accepts no URL, credential, prompt, tool output, cursor, or response identifier.

```bash
node scripts/node/provider-timing-benchmark/benchmark.js --dry-run
node scripts/node/provider-timing-benchmark/benchmark.js --schema
node scripts/node/provider-timing-benchmark/benchmark.js --input /path/to/sanitized-samples.json
```

Input is a JSON array capped at 1 MiB and 200 samples per variant. Each item contains exactly `variant`, `sample_index`, and `metrics_ms`; timing values are nullable milliseconds. A real-network collector is intentionally outside this development packet. After the user performs equivalent direct and gateway invocations, only the metadata-only timing samples should be passed here.

The result reports sample count, P50, and P95 for total, ingress, mapping, flow, queue, connect, upstream, and flush. Ingress is elapsed time to the first event observed by AI Native; other fields are owner-local durations. `null` means that stage's owner could not observe it accurately; it must not be replaced with a guessed zero.
