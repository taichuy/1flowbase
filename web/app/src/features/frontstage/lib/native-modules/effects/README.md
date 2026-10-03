# Block effects

- `cssinjs-runtime.tsx` exposes `StyleProvider` with the installed package's prop types. Local CSS options are supported; the Block runtime retains ownership of the cache and injection container. Replacing either with an external owner fails explicitly.
- `happy-work-runtime.tsx` exposes the upstream `HappyProvider` API and reuses `@ant-design/happy-work-theme@2.0.0/es/DotEffect`. The pinned internal entry preserves the official particle algorithm. A React portal keeps Block context and mounts into the existing overlay host; layout invalidation and unmount remove effects.
- `web/patches/@ant-design__happy-work-theme@2.0.0.patch` clears the upstream delayed particle task and target attribute on unmount in both ESM and CommonJS builds. Remove the patch when the installed upstream version provides equivalent cleanup, and rerun the effect lifecycle tests before updating the pinned internal import.
- Keep new runtime exports and editor types aligned with the module registry. Do not expose the upstream body-mounted provider or independent React roots to Block source.

Upstream: https://github.com/ant-design/happy-work-theme (MIT).
