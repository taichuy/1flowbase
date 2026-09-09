# Dialog focus in native Block surfaces

Issue: https://github.com/taichuy/1flowbase/issues/2019

Pinned patches: `@rc-component/dialog@1.10.0` and `@rc-component/util@1.12.0`.
Both ESM (`es`) and CommonJS (`lib`) distributions are patched; package versions
and public APIs are unchanged.

## Problem

A document's activeElement is a shadow host when an input inside that host has
focus. The dialog's opening-motion callback therefore focused the dialog over
an already active textarea; closing also restored the host instead of the
trigger. The shared focus lock treated that textarea as outside its scope.
Furthermore, focus transitions within the same shadow tree do not necessarily
reach the window listener after event retargeting.

## Adaptation

- Resolve the active control through open shadow roots.
- Compare focus ownership through composed ancestry.
- Listen for focus changes on a lock's ShadowRoot as well as window; keep the
  root listener until its last lock unregisters.
- Restore the trigger on direct Modal unmount after child focus locks release;
  skip restoration when the trigger was removed or StrictMode remounted the dialog.
- Retain existing lock ordering, ignored popup elements, Tab cycling and the
  normal DOM path. No document.activeElement override or Block-side event code.

Closed shadow roots cannot be inspected from the document; the native Block
runtime currently uses open roots. This patch does not claim to add general
cross-document focus management or enumerate every nested widget's Tab order.

## Validation and removal

Tests live in `web/app/src/features/frontstage/_tests/page-canvas/keyboard/`.
The canvas consumer uses the ESM Modal; dependency tests load the installed CJS
focus lock. Browser acceptance covers early typing during opening motion,
multiline text, template Select, modal dismissal and focus restoration on the
AI Gateway page. Synthetic composition tests are not OS input-method testing.

On dependency upgrade, compare upstream implementations of Dialog focus and
util/Dom/focus. Remove both patches only after the ownership and lifecycle tests
and the early-typing browser case pass against the unpatched versions. Do not
silently disable a patch to make installation succeed.
