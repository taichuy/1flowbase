import { describe, expect, test, vi } from 'vitest';

import type { BlockContext } from '../../../page-protocol/src/index';

import {
  isBlockModule,
  isBlockResult,
  type BlockModule,
  type BlockResult
} from '../index';

const blockContext = {
  currentUser: null,
  workspace: { id: 'workspace-1', name: 'Workspace' },
  application: { id: 'application-1', name: 'Application' },
  page: { id: 'page-1', route: '/demo', title: 'Demo' },
  inputs: {},
  params: {},
  props: {},
  state: {},
  patch: vi.fn(),
  interfaces: { call: vi.fn() },
  events: { emit: vi.fn() },
  theme: { mode: 'light', tokens: {} },
  ui: { locale: 'en_US', density: 'comfortable' }
} satisfies BlockContext;

describe('BlockModule', () => {
  test('AC-001 accepts the native default module shape and runs main', async () => {
    const module = {
      async main(ctx) {
        return {
          view: {
            primitive: 'Text',
            props: { children: ctx.page.title }
          },
          outputs: { ready: true }
        };
      }
    } satisfies BlockModule;

    expect(isBlockModule(module)).toBe(true);
    await expect(module.main(blockContext)).resolves.toEqual({
      view: {
        primitive: 'Text',
        props: { children: 'Demo' }
      },
      outputs: { ready: true }
    });
  });

  test.each([
    ['null', null],
    ['function export', () => undefined],
    ['missing main', {}],
    ['non-function main', { main: 'render' }],
    ['legacy render', { render: () => ({ primitive: 'Text' }) }],
    ['module metadata', { title: 'duplicate truth', main: () => undefined }]
  ])('AC-001 rejects an invalid module: %s', (_label, value) => {
    expect(isBlockModule(value)).toBe(false);
  });

  test('requires BlockResult outputs to be a plain object', () => {
    const legal = {
      view: { primitive: 'Text' },
      outputs: { total: 1 }
    } satisfies BlockResult;

    expect(isBlockResult(legal)).toBe(true);
    expect(
      isBlockResult({ view: { primitive: 'Text' }, outputs: [] })
    ).toBe(false);
    expect(isBlockResult({ view: { primitive: 'Text' } })).toBe(false);
  });
});
