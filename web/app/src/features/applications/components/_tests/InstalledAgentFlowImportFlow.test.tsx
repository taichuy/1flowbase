import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import type { ConsoleInstalledApplicationExtensionPreview } from '@1flowbase/api-client';
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

const api = vi.hoisted(() => ({
  previewInstalledApplicationExtension: vi.fn(),
  importInstalledApplicationExtension: vi.fn()
}));
vi.mock('../../api/applications', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../api/applications')>()),
  ...api
}));

import { createRoot } from 'react-dom/client';

import { AppProviders } from '../../../../app/AppProviders';
import { InstalledAgentFlowImportFlow } from '../InstalledAgentFlowImportFlow';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

function preview(
  id = 'installation-a'
): ConsoleInstalledApplicationExtensionPreview {
  return {
    extension_installation_id: id,
    application_status: 'not_applied',
    integrity_warnings: [
      { code: 'unsigned', message: `Warning ${id}`, overridable: true }
    ],
    required_integrity_override: null,
    preview: {
      schema_version: '1flowbase.application-template/v1',
      application: {
        application_type: 'agent_flow',
        name: `Template ${id}`,
        description: `Description ${id}`,
        icon: null,
        icon_type: null,
        icon_background: null
      },
      document: createDefaultAgentFlowDocument({ flowId: 'flow-a' }),
      dependencies: [],
      unresolved_nodes: []
    }
  };
}

function imported() {
  const source = preview().preview;
  return {
    application: {
      ...source.application,
      id: 'server-created-id',
      created_by: 'user-a',
      updated_at: '2026-11-01T00:00:00Z'
    },
    orchestration: {
      flow_id: 'flow-a',
      draft: {
        id: 'draft-a',
        flow_id: 'flow-a',
        document: source.document,
        updated_at: '2026-11-01T00:00:00Z'
      },
      messages: [],
      versions: [],
      autosave_interval_seconds: 30,
      user_protection_limit: 10
    },
    preview: source
  };
}

function mount(installationId: string | null = 'installation-a') {
  const onClose = vi.fn();
  const onImported = vi
    .fn<(id: string) => Promise<void>>()
    .mockResolvedValue(undefined);
  const element = (id: string | null) => (
    <AppProviders>
      <InstalledAgentFlowImportFlow
        installationId={id}
        csrfToken="csrf-a"
        onClose={onClose}
        onImported={onImported}
      />
    </AppProviders>
  );
  const result = render(element(installationId));
  return {
    ...result,
    onClose,
    onImported,
    switchTo: (id: string | null) => result.rerender(element(id))
  };
}

async function confirm() {
  const input = await screen.findByRole('textbox', { name: '应用名称' });
  fireEvent.change(input, { target: { value: '  Edited name  ' } });
  fireEvent.click(screen.getByRole('button', { name: '导入应用' }));
}

describe('installed template import through the real Modal', () => {
  beforeEach(() => {
    api.previewInstalledApplicationExtension.mockReset();
    api.importInstalledApplicationExtension.mockReset();
    api.previewInstalledApplicationExtension.mockResolvedValue(preview());
    api.importInstalledApplicationExtension.mockResolvedValue(imported());
  });
  afterEach(cleanup);

  test.each([null, ''])(
    'I-01: inactive installation %s does not request or open',
    (id) => {
      mount(id);
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
      expect(api.previewInstalledApplicationExtension).not.toHaveBeenCalled();
      expect(api.importInstalledApplicationExtension).not.toHaveBeenCalled();
    }
  );

  test.each([
    { codes: null },
    { codes: [] },
    { codes: ['unsigned', 'changed', 'unsigned'] }
  ])(
    'I-02/03: renders the preview and preserves acknowledgement $codes',
    async ({ codes }) => {
      const source = preview();
      source.required_integrity_override =
        codes === null
          ? null
          : {
              warnings: codes.map((code) => ({
                code,
                message: code,
                overridable: true
              })),
              compatibility: null
            };
      api.previewInstalledApplicationExtension.mockResolvedValue(source);
      const mounted = mount();
      expect(
        await screen.findByDisplayValue('Template installation-a')
      ).toBeInTheDocument();
      expect(
        screen.getByText('Description installation-a')
      ).toBeInTheDocument();
      expect(screen.getByText('Warning installation-a')).toBeInTheDocument();
      expect(
        api.previewInstalledApplicationExtension
      ).toHaveBeenCalledExactlyOnceWith('installation-a');
      expect(api.importInstalledApplicationExtension).not.toHaveBeenCalled();
      await confirm();
      await waitFor(() =>
        expect(mounted.onImported).toHaveBeenCalledExactlyOnceWith(
          'server-created-id'
        )
      );
      expect(
        api.importInstalledApplicationExtension
      ).toHaveBeenCalledExactlyOnceWith(
        'installation-a',
        {
          name: 'Edited name',
          description: 'Description installation-a',
          ...(codes === null
            ? {}
            : {
                integrity_override: {
                  reason: 'user_confirmed',
                  acknowledged_warnings: codes
                }
              })
        },
        'csrf-a'
      );
      expect(mounted.onClose).not.toHaveBeenCalled();
    }
  );

  test.each(['resolve', 'reject'] as const)(
    'I-04: ignores stale preview %s after an ID switch',
    async (completion) => {
      const stale = deferred<ConsoleInstalledApplicationExtensionPreview>();
      const current = deferred<ConsoleInstalledApplicationExtensionPreview>();
      api.previewInstalledApplicationExtension
        .mockReturnValueOnce(stale.promise)
        .mockReturnValueOnce(current.promise);
      const mounted = mount();
      mounted.switchTo('installation-b');
      await act(async () => {
        if (completion === 'resolve') stale.resolve(preview());
        else stale.reject(new Error('obsolete failure'));
      });
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
      expect(mounted.onClose).not.toHaveBeenCalled();
      await act(async () => {
        current.resolve(preview('installation-b'));
      });
      expect(
        await screen.findByDisplayValue('Template installation-b')
      ).toBeInTheDocument();
      expect(
        screen.queryByText('Warning installation-a')
      ).not.toBeInTheDocument();
      await confirm();
      await waitFor(() =>
        expect(api.importInstalledApplicationExtension).toHaveBeenCalledOnce()
      );
      expect(api.importInstalledApplicationExtension.mock.calls[0]).toEqual([
        'installation-b',
        { name: 'Edited name', description: 'Description installation-b' },
        'csrf-a'
      ]);
    }
  );

  test.each(['resolve', 'reject'] as const)(
    'I-04: ignores preview %s after unmount',
    async (completion) => {
      const pending = deferred<ConsoleInstalledApplicationExtensionPreview>();
      api.previewInstalledApplicationExtension.mockReturnValue(pending.promise);
      const mounted = mount();
      mounted.unmount();
      await act(async () => {
        if (completion === 'resolve') pending.resolve(preview());
        else pending.reject(new Error('unmounted failure'));
      });
      expect(mounted.onClose).not.toHaveBeenCalled();
      expect(mounted.onImported).not.toHaveBeenCalled();
      expect(api.importInstalledApplicationExtension).not.toHaveBeenCalled();
    }
  );

  test('I-05: confirmation stays busy until the server-id callback completes', async () => {
    const request = deferred<ReturnType<typeof imported>>();
    const callback = deferred<void>();
    api.importInstalledApplicationExtension.mockReturnValue(request.promise);
    const mounted = mount();
    mounted.onImported.mockReturnValue(callback.promise);
    await confirm();
    const button = screen.getByRole('button', { name: '导入应用' });
    expect(button).toHaveClass('ant-btn-loading');
    expect(mounted.onImported).not.toHaveBeenCalled();
    await act(async () => {
      request.resolve(imported());
    });
    expect(mounted.onImported).toHaveBeenCalledExactlyOnceWith(
      'server-created-id'
    );
    expect(button).toHaveClass('ant-btn-loading');
    await act(async () => {
      callback.resolve();
    });
    await waitFor(() => expect(button).not.toHaveClass('ant-btn-loading'));
    expect(api.importInstalledApplicationExtension).toHaveBeenCalledOnce();
  });

  test.each(['request', 'callback'] as const)(
    'I-05: %s errors are displayed and release busy without retry',
    async (failure) => {
      const mounted = mount();
      if (failure === 'request')
        api.importInstalledApplicationExtension.mockRejectedValue(
          new Error('import failed')
        );
      else mounted.onImported.mockRejectedValue(new Error('import failed'));
      await confirm();
      expect(await screen.findByText('import failed')).toBeInTheDocument();
      await waitFor(() =>
        expect(
          screen.getByRole('button', { name: '导入应用' })
        ).not.toHaveClass('ant-btn-loading')
      );
      expect(api.importInstalledApplicationExtension).toHaveBeenCalledOnce();
      expect(mounted.onImported).toHaveBeenCalledTimes(
        failure === 'request' ? 0 : 1
      );
      expect(mounted.onClose).not.toHaveBeenCalled();
    }
  );

  test('I-06: current committed Modal confirmation uses its render acknowledgement before new preview commits', async () => {
    const container = document.createElement('div');
    document.body.append(container);
    const root = createRoot(container);
    const observed = deferred<void>();
    const onClose = vi.fn();
    const onImported = vi.fn().mockResolvedValue(undefined);
    const element = (id: string) => (
      <AppProviders>
        <InstalledAgentFlowImportFlow
          installationId={id}
          csrfToken="csrf-a"
          onClose={onClose}
          onImported={onImported}
        />
      </AppProviders>
    );
    const source = (id: string) => {
      const result = preview(id);
      result.required_integrity_override = {
        warnings: [{ code: id, message: id, overridable: true }],
        compatibility: null
      };
      return result;
    };
    api.previewInstalledApplicationExtension.mockImplementation(
      (id: string) => {
        if (id === 'installation-b') {
          // Q1 is queued before Flow attaches then; Q2 runs after the entire
          // successful preview projection, but before its queued state commits.
          // This is a programmatic microtask event, NOT a physical-user race.
          queueMicrotask(() =>
            queueMicrotask(() => {
              try {
                expect(
                  screen.getByDisplayValue('Template installation-a')
                ).toBeInTheDocument();
                screen.getByRole('button', { name: '导入应用' }).click();
                expect(
                  api.importInstalledApplicationExtension
                ).toHaveBeenCalledExactlyOnceWith(
                  'installation-b',
                  {
                    name: 'Template installation-a',
                    description: 'Description installation-a',
                    integrity_override: {
                      reason: 'user_confirmed',
                      acknowledged_warnings: ['installation-a']
                    }
                  },
                  'csrf-a'
                );
                observed.resolve();
              } catch (error) {
                observed.reject(error);
              }
            })
          );
        }
        return Promise.resolve(source(id));
      }
    );
    // Keep the import pending; this assertion concerns request construction,
    // not a response/callback or a server-side cancellation contract.
    api.importInstalledApplicationExtension.mockReturnValue(
      new Promise(() => {})
    );
    const environment = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    const originalActEnvironment = environment.IS_REACT_ACT_ENVIRONMENT;
    try {
      await act(async () => {
        root.render(element('installation-a'));
      });
      expect(screen.getByRole('button', { name: '导入应用' })).not.toHaveClass(
        'ant-btn-loading'
      );
      // Do not wrap B in act: it would eagerly flush the very pending-commit
      // window under test. No scheduler mocks, forced lanes or retained callback.
      environment.IS_REACT_ACT_ENVIRONMENT = false;
      root.render(element('installation-b'));
      await observed.promise;
    } finally {
      environment.IS_REACT_ACT_ENVIRONMENT = true;
      await act(async () => {
        root.unmount();
      });
      environment.IS_REACT_ACT_ENVIRONMENT = originalActEnvironment;
      container.remove();
    }
  });

  test('I-06: the real Modal blocks an empty draft and cancel never imports', async () => {
    const mounted = mount();
    const input = await screen.findByRole('textbox', { name: '应用名称' });
    fireEvent.change(input, { target: { value: '   ' } });
    expect(screen.getByRole('button', { name: '导入应用' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: /取\s*消/ }));
    expect(mounted.onClose).toHaveBeenCalledOnce();
    expect(api.importInstalledApplicationExtension).not.toHaveBeenCalled();
  });

  test.each(['warnings', 'challenge'] as const)(
    'I-06: malformed fulfilled %s retains the existing partial-preview error path',
    async (field) => {
      // Deliberately exercise invalid JSON: do not silently change partial queued
      // updates into an atomic replacement while slimming the loaded state.
      const source = preview();
      if (field === 'warnings')
        source.integrity_warnings = [
          null
        ] as unknown as typeof source.integrity_warnings;
      else
        source.required_integrity_override = {
          warnings: null,
          compatibility: null
        } as unknown as NonNullable<typeof source.required_integrity_override>;
      api.previewInstalledApplicationExtension.mockResolvedValue(source);
      const mounted = mount();
      expect(
        await screen.findByDisplayValue('Template installation-a')
      ).toBeInTheDocument();
      await waitFor(() => expect(mounted.onClose).toHaveBeenCalledOnce());
      expect(
        screen.getByText('Description installation-a')
      ).toBeInTheDocument();
      expect(Boolean(screen.queryByText('Warning installation-a'))).toBe(
        field === 'challenge'
      );
      await waitFor(() =>
        expect(
          screen.getByRole('button', { name: '导入应用' })
        ).not.toHaveClass('ant-btn-loading')
      );
      expect(api.importInstalledApplicationExtension).not.toHaveBeenCalled();
    }
  );

  test('I-04: current preview rejection closes without importing', async () => {
    api.previewInstalledApplicationExtension.mockRejectedValue(
      new Error('preview failed')
    );
    const mounted = mount();
    expect(await screen.findByText('preview failed')).toBeInTheDocument();
    expect(mounted.onClose).toHaveBeenCalledOnce();
    expect(api.importInstalledApplicationExtension).not.toHaveBeenCalled();
  });
});
