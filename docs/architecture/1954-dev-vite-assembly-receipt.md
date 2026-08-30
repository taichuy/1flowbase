# #1954 公网 Vite 开发环境 Assembly Receipt

## Candidate

- Branch: `dev`
- Product assembly: `b208c67679bd053e4778743c589eebf9cd5cd4f9`
- Public entry: `https://1flowbase.taichuy.cn/`
- Browser: system Chromium `/usr/bin/google-chrome`, fresh incognito context
- Runtime mode: Vite development server with source maps and HMR

## Result

`QA_PASS` for the #1954 Dev Acceptance scope.

The public cold profile reached `/sign-in` in `7801 ms` with `48` requests,
`41` module requests, `0` failed requests, `0` actionable console/page errors,
and `0` pending critical modules. An independent post-restart incognito run mounted
`#root` with `4896` bytes of rendered markup and exposed
`[data-testid="builtin-password-sign-in"]` in `6928 ms`.

The authenticated profiles also passed for Frontstage, Settings, and Workflow
Editor. HMR round-trip was `11 ms`; an isolated fresh dependency cache rebuilt
and exposed the Frontstage ready selector in `12678 ms` without module failures.

## Architecture Result

- The bootstrap starts single-flight session discovery before loading the React
  application graph.
- Anonymous and authenticated runtimes have separate router, provider, and i18n
  boundaries. The public password form does not require the complete Ant Design
  application runtime.
- Host Ant Design icons use deterministic leaf imports. Native runtime metadata
  remains separate from executable module loaders.
- Narrow package exports exist for auth, theme provider, loading shell, and source
  contract consumers; production aliases do not define the package contract alone.
- Fresh Vite caches stay inside the application module-resolution domain, and
  `optimizeDeps` does not force unresolved transitive packages.

## Acceptance Settlement

| AC     | Status                | Evidence                                                                                                                                           |
| ------ | --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC-001 | GREEN                 | Real authenticated Frontstage, Settings, Workflow cold/warm/recovery profiles; no JS/module failure                                                |
| AC-002 | GREEN                 | Ant icon barrel controlled negative and `0` icon leaf requests on public cold path                                                                 |
| AC-003 | GREEN                 | Native module contract tests and single-flight/lazy registry regression                                                                            |
| AC-004 | GREEN                 | Candidate-bound static graph and browser Resource Timing receipt                                                                                   |
| AC-005 | GREEN                 | Cache identity fixtures and successful isolated fresh-cache profile                                                                                |
| AC-006 | GREEN                 | Scanning/Optimizing/Warming/Ready/Degraded lifecycle and pre-ready gate fixtures                                                                   |
| AC-007 | GREEN                 | Bounded warmup targets use the split bootstrap and measured route owners                                                                           |
| AC-008 | GREEN (revised scope) | No new Access/security product was added per user decision; existing configurable host/origin boundary remains and the public tunnel was exercised |
| AC-009 | GREEN                 | Absolute budgets and Median/MAD fixture gates pass                                                                                                 |
| AC-010 | GREEN                 | 14 machine-readable cold/warm/HMR/cache-rebuild/concurrent/recovery profiles                                                                       |
| AC-011 | GREEN                 | Four package declaration builds and production application build pass; no backend/API/database/plugin change                                       |
| AC-012 | GREEN                 | Final frozen matrix has no unrun scenario                                                                                                          |

## Verification

- Governance Node fixtures: `18/18`
- Auth and Native targeted Vitest: `55/55`
- Page Runtime package tests: `155/155`
- Native module tests: `25/25`
- Package declaration builds: `4/4`
- Production application build: PASS
- Style boundary affected by `globals.css`: `18/18`
- i18n hygiene: exit `0`; existing unused-key warnings retained without changing user text
- `git diff --check`: PASS

Machine-readable receipt:
`tmp/test-governance/1954-candidate-b208c6767/dev-experience-profile.json`.
Desktop and mobile evidence:
`tmp/test-governance/1954-public-incognito-sign-in.png` and
`tmp/test-governance/1954-public-incognito-sign-in-mobile.png`.

## Non-blocking Baseline Warning

The full frontend lint command still reports one pre-existing error in
`native-anchor-runtime.test.tsx` (`jest-dom/prefer-to-have-style`) and 40 existing
warnings. That file is outside the #1954 diff; all #1954 files are free of lint
errors. Existing production large-chunk warnings are unchanged.

No push, Issue closure, or protected baseline update was performed by this
assembly.
