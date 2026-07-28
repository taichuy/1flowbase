import { describe, expect, it } from 'vitest';

import { validateI18nTextRef, type FlowBinding } from '../index';

describe('typed i18n_text binding contract', () => {
  it('AC-006 preserves a canonical multi-level module and English msgid', () => {
    const binding = {
      kind: 'i18n_text',
      value: {
        module: '@org/agent-flow/messages',
        key: 'Welcome, {name}!'
      }
    } satisfies FlowBinding;

    expect(validateI18nTextRef(binding.value)).toEqual({ ok: true });
    expect(JSON.parse(JSON.stringify(binding))).toEqual(binding);
  });

  it.each([
    [{ module: '@org/messages' }, 'invalid_i18n_shape'],
    [{ key: 'Welcome' }, 'invalid_i18n_shape'],
    [
      { module: '@org/messages', key: 'Welcome', extra: true },
      'invalid_i18n_shape'
    ],
    [{ module: 7, key: 'Welcome' }, 'invalid_i18n_shape'],
    [{ module: '@org/messages', key: 7 }, 'invalid_i18n_shape'],
    [{ module: 'org/messages', key: 'Welcome' }, 'invalid_i18n_module'],
    [{ module: '@Org/messages', key: 'Welcome' }, 'invalid_i18n_module'],
    [{ module: '@org/messages', key: '   ' }, 'blank_i18n_key'],
    [
      { module: '@org/messages', key: 'Welcome {{node-start.query}}' },
      'workflow_template_conflict'
    ],
    [
      { module: '@org/messages', key: '<strong>Welcome</strong>' },
      'non_plain_i18n_text'
    ],
    [
      { module: '@org/messages', key: 'javascript:alert(1)' },
      'non_plain_i18n_text'
    ],
    [
      { module: '@org/messages', key: 'Welcome {user.name}' },
      'invalid_named_placeholder'
    ]
  ] satisfies Array<[unknown, string]>)(
    'AC-010 rejects invalid ref %#',
    (reference, reason) => {
      expect(validateI18nTextRef(reference)).toEqual({ ok: false, reason });
    }
  );
});
