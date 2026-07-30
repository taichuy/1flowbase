import { describe, expect, it } from 'vitest';

import { validateI18nTextRef, type FlowBinding } from '../index';

describe('typed i18n_text binding contract', () => {
  it('AC-006 preserves a global immutable English key', () => {
    const binding = {
      kind: 'i18n_text',
      value: {
        key: 'Welcome, {name}!'
      }
    } satisfies FlowBinding;

    expect(validateI18nTextRef(binding.value)).toEqual({ ok: true });
    expect(JSON.parse(JSON.stringify(binding))).toEqual(binding);
  });

  it.each([
    [{}, 'invalid_i18n_shape'],
    [{ key: 'Welcome', extra: true }, 'invalid_i18n_shape'],
    [{ key: 7 }, 'invalid_i18n_shape'],
    [{ key: '   ' }, 'blank_i18n_key'],
    [
      { key: 'Welcome {{node-start.query}}' },
      'workflow_template_conflict'
    ],
    [
      { key: '<strong>Welcome</strong>' },
      'non_plain_i18n_text'
    ],
    [
      { key: 'javascript:alert(1)' },
      'non_plain_i18n_text'
    ],
    [
      { key: 'Welcome {user.name}' },
      'invalid_named_placeholder'
    ]
  ] satisfies Array<[unknown, string]>)(
    'AC-010 rejects invalid ref %#',
    (reference, reason) => {
      expect(validateI18nTextRef(reference)).toEqual({ ok: false, reason });
    }
  );
});
