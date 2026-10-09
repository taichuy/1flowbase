# React block translations

[简体中文](README_CN.md)

The host provides `ctx.i18n` to native React blocks in the page canvas and JSX
Studio run panel. It loads the backend runtime catalog for the current app
language. Blocks do not need to fetch catalogs or import a translation library.

```tsx
import type { BlockComponentProps } from '@1flowbase/block-sdk';

export default function Block({ ctx }: BlockComponentProps) {
  return <span>{ctx.i18n.t('Account')}</span>;
}
```

- `t(key)` uses a literal, globally unique backend catalog key. Current official
  keys are English source text, such as `Account`; dots and colons remain part of
  the key. Console frontend resource keys such as `auto.*` are a separate catalog.
- `t(key, { defaultValue, values })` supports an optional fallback and
  `{{variable}}` interpolation. React escapes the rendered text.
- An unknown key returns `defaultValue`, or the key if no fallback is supplied.
  While loading or after a failed request, the same fallback applies.
- `locale` matches `ctx.ui.locale`. `status` is `loading`, `ready`, or `error`.
- Language changes update the block context and re-render the component without
  resetting its local Hook state. Use `t` during render instead of saving its
  output in module scope or initial state.
- The existing runtime catalog endpoint requires `i18n.catalog.view` and the
  root workspace. Other callers receive no catalog; the host reports `error`.
  This SDK does not extend backend permissions.

```tsx
ctx.i18n.t('Hello {{name}}', {
  values: { name: 'Ada' },
  defaultValue: 'Hello {{name}}'
});
```

Translation snapshots are cached by user, workspace, permissions and locale for
five minutes, with normal query revalidation on focus. Catalog edits do not
provide push updates. Run-panel contexts created without the page host report
`error` and support fallback text only.
