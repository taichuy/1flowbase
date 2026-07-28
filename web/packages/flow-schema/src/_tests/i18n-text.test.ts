import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { validateI18nTextRef, type FlowBinding } from '../index';

const moduleIdentityFixture = JSON.parse(
  readFileSync(
    new URL(
      '../../../../../api/crates/domain/src/_tests/fixtures/i18n-module-identity.json',
      import.meta.url
    ),
    'utf8'
  )
) as { valid: string[]; invalid: string[] };

describe('typed i18n_text binding contract', () => {
  it.each(moduleIdentityFixture.valid)(
    'accepts canonical module identity %s',
    (module) => {
      expect(validateI18nTextRef({ module, key: 'Continue' })).toEqual({
        ok: true
      });
    }
  );

  it.each(moduleIdentityFixture.invalid)(
    'rejects noncanonical module identity %s',
    (module) => {
      expect(validateI18nTextRef({ module, key: 'Continue' })).toEqual({
        ok: false,
        reason: 'invalid_i18n_module'
      });
    }
  );

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
    [{ module: '@org/group/messages' }, 'invalid_i18n_shape'],
    [{ key: 'Welcome' }, 'invalid_i18n_shape'],
    [
      { module: '@org/group/messages', key: 'Welcome', extra: true },
      'invalid_i18n_shape'
    ],
    [{ module: 7, key: 'Welcome' }, 'invalid_i18n_shape'],
    [{ module: '@org/group/messages', key: 7 }, 'invalid_i18n_shape'],
    [{ module: '@org/group/messages', key: '   ' }, 'blank_i18n_key'],
    [
      { module: '@org/group/messages', key: 'Welcome {{node-start.query}}' },
      'workflow_template_conflict'
    ],
    [
      { module: '@org/group/messages', key: '<strong>Welcome</strong>' },
      'non_plain_i18n_text'
    ],
    [
      { module: '@org/group/messages', key: 'javascript:alert(1)' },
      'non_plain_i18n_text'
    ],
    [
      { module: '@org/group/messages', key: 'Welcome {user.name}' },
      'invalid_named_placeholder'
    ]
  ] satisfies Array<[unknown, string]>)(
    'AC-010 rejects invalid ref %#',
    (reference, reason) => {
      expect(validateI18nTextRef(reference)).toEqual({ ok: false, reason });
    }
  );
});
