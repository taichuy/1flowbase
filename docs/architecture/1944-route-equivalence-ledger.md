# #1944 Route Equivalence Ledger

The approved migration set remains exactly four routes. The machine fixture `interface_route_equivalence.1944.json` binds each route to its current Kernel/principal source and pre-existing behavior regression source across allow/deny, row scope, mutation, DTO, status/error, stream order, transaction/outbox, Runtime dispatch, audit and receipt observations.

No production double write, legacy fallback or second route registration was introduced. Compatibility HTTP/WebSocket, `/api/ex`, public sign-in, Internal/Background and dynamic route expansion remain explicit gaps and were not migrated. Their existing external behavior stays in the centralized regression batch.
