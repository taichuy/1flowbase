# storage-ephemeral

## Capability Boundary

`storage-ephemeral` is the non-durable infrastructure layer for short-lived coordination data. It carries session adapters, ephemeral key/value storage, and future workflow coordination primitives without turning Redis vocabulary into the public architectural boundary.

## Built-in Backends

- `memory` is the default built-in backend for single-node development and local execution.
- `redis` remains an optional backend for future multi-node deployment needs.

## Workflow Primitives

The crate exposes `LeaseStore` and `WakeupSignalBus` as reusable ephemeral coordination interfaces. Current implementations stay in-process and are intentionally narrow so workflow durability continues to live in PostgreSQL.

## Why Host Extensions Cannot Provide Session Backends Yet

Host extensions are activated after `ApiState` construction and after the session backend has already been chosen. They cannot currently supply `SessionStore`, `EphemeralKvStore`, or other early startup infrastructure providers.

## Future Infra Provider Bootstrap Contract

A future infra-provider bootstrap contract must load before `app_from_config()` builds core state. That earlier seam is the only safe place to register optional session backends or workflow coordination providers without reintroducing hard-coded startup dependencies.
