# Native canonical projection

The normal tail path retains canonical bodies in the existing message table.
It reads new source facts and old bodies only for overlapping keys. All checks
run under the existing conversation lock, before any projection write.

The ordered merge remains **delta then old**. An unequal representative, owner
move, changed special/lifecycle row, non-tail rank, original request sidecar,
invalidated source or incompatible version takes the full retained-source path.
No-body-change refreshes publish progress without replacing existing messages.

Native freshness and the published run revision belong to the existing progress
row. A message row's `source_revision` and embedded `_log_source_revision` record
its materialization revision. Trace and native exchange readers overlay the
published revision after lossless JSON restoration. This preserves their prior
public metadata without copying every historical JSON body for each new fact.
Legacy freshness still checks row revisions. There is no new table or cache.

The bounded abstract merge review does not prove SQL, Rust or transaction
behavior. Integration tests compare the real incremental writer with full
repair, check unchanged tuple identities and exercise failure/cancellation.
The opt-in `native_projection_cost_benchmark` invokes `append_runtime_events`;
its fixture counters and optional dedicated-cluster statistics are test-only.
