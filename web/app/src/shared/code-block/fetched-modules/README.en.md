# Verified frontend modules

The host supplies `code_modules` from an authorized plugin contribution catalog. User component source never grants plugin permissions. A component record is a discoverable code resource, not authorization to execute its modules.

- `binding: host` references registered host modules and declared exports, without assets.
- `binding: fetched` declares one `browser_module` and optional `shadow_style` assets. Every executable and stylesheet byte is checked against its manifest SHA-256 before loading.
- Existing self-contained ESM may export the declared members directly.
- Bundles that share host React, Ant Design, or other dependencies may export `createFrontendModule(hostModules)`. Its argument contains only host sources and exports explicitly declared in the same `code_modules` inventory, preserving host reference identity. The factory returns an object or a Promise of an object containing every declared export.

```js
export function createFrontendModule(hostModules) {
  const React = hostModules.react;
  return {
    Example: () => React.createElement('span', null, 'Example')
  };
}
```

Build tooling may wrap a CommonJS bundle in this factory and restrict `require` to own keys of `hostModules`. Do not bundle another React copy or depend on global module variables, external CDNs, or page import maps.

The caller owns each loader epoch. Identity, workspace, catalog version, or authorization changes dispose the old loader, abort its fetches, and release Blob URLs. A new epoch uses the backend-authorized catalog and asset endpoint again. The frontstage artifact cache includes full module declarations in its policy identity, preventing an upgrade from reusing old factories. Trusted-host JavaScript is not a security sandbox: disabling a plugin cannot undo arbitrary effects already executed in a browser. The backend still rejects new unauthorized asset reads.

[中文](./README.md)

Frontstage retains each fetched module's installation owner and its own host-export inventory. Independent plugins may request different valid subsets of one host module without conflict. A factory never receives another plugin's declared dependencies. Conflicting owners, bytes, versions, or dependencies for the same fetched source are still rejected.
