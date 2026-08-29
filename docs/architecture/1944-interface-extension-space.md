# #1944 Interface Extension Space

| Tier | Definition | Authentication adapter | Authorization / Admission | Before / After | Handler | Failure / Completion | Isolation |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Built-in | allowed | allowed | allowed | allowed; mutation permission explicit | allowed; exactly one effective target | allowed | trusted in-process |
| HostExtension | allowed | allowed | allowed | allowed; mutation permission explicit | allowed; exactly one effective target | allowed | trusted in-process |
| RuntimeExtension | allowed with permission | forbidden | allowed with typed facts | allowed with permission | allowed with permission | allowed with typed facts | process/wire |
| CapabilityPlugin | allowed with permission | forbidden | allowed with typed facts | allowed with permission | allowed with permission | allowed with typed facts | process/wire |

Every registration compiles a point, permission, interface scope, isolation mode and a subset of point-specific typed facts. Authentication receives no generally distributable fact set: raw credentials stay inside the trusted adapter implementation. `interface.before` can mutate only with `MutateInput`; observation permission cannot mutate. Handler compilation rejects zero or multiple effective targets. Completion receives terminal/invocation facts only and has no Domain Event or outbox port.
