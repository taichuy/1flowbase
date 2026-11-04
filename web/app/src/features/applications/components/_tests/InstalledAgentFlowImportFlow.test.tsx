import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import type { ConsoleInstalledApplicationExtensionPreview } from '@1flowbase/api-client';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const api = vi.hoisted(() => ({
  previewInstalledApplicationExtension: vi.fn(),
  importInstalledApplicationExtension: vi.fn()
}));
vi.mock('../../api/applications', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../api/applications')>()),
  ...api
}));

// Observe the callback value forwarded to the actual Modal, without replacing
// its DOM, loading, accessibility, form or event behavior with a test double.
const modalObservation = vi.hoisted(() => ({
  onImport: undefined as (() => void) | undefined
}));
vi.mock('../ApplicationTemplateImportModal', async (importOriginal) => {
  const actual =
    await importOriginal<typeof import('../ApplicationTemplateImportModal')>();
  const ActualModal = actual.ApplicationTemplateImportModal;
  return {
    ...actual,
    ApplicationTemplateImportModal: function ObservedImportModal(
      props: Parameters<typeof ActualModal>[0]
    ) {
      modalObservation.onImport = props.onImport;
      return <ActualModal {...props} />;
    }
  };
});

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
  const view = render(element(installationId));
  return {
    ...view,
    onClose,
    onImported,
    switchTo: (id: string | null) => view.rerender(element(id))
  };
}

async function confirm() {
  const input = await screen.findByRole('textbox', { name: '应用名称' });
  fireEvent.change(input, { target: { value: '  Edited name  ' } });
  const button = screen.getByRole('button', { name: '导入应用' });
  fireEvent.click(button);
  return button;
}

describe('installed template import through the real Modal', () => {
  beforeEach(() => {
    modalObservation.onImport = undefined;
    api.previewInstalledApplicationExtension.mockReset();
    api.importInstalledApplicationExtension.mockReset();
    api.previewInstalledApplicationExtension.mockResolvedValue(preview());
    api.importInstalledApplicationExtension.mockResolvedValue(imported());
  });

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
    const button = await confirm();
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
      const request = deferred<ReturnType<typeof imported>>();
      const callback = deferred<void>();
      api.importInstalledApplicationExtension.mockReturnValue(request.promise);
      const mounted = mount();
      mounted.onImported.mockReturnValue(callback.promise);
      await confirm();
      if (failure === 'request') {
        await act(async () => {
          request.reject(new Error('import failed'));
        });
      } else {
        await act(async () => {
          request.resolve(imported());
        });
        expect(mounted.onImported).toHaveBeenCalledExactlyOnceWith(
          'server-created-id'
        );
        await act(async () => {
          callback.reject(new Error('import failed'));
        });
      }
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

  test('I-06: an explicitly captured render callback keeps its preview acknowledgement after another preview commits', async () => {
    api.previewInstalledApplicationExtension.mockImplementation(
      (id: string) => {
        const result = preview(id);
        result.required_integrity_override = {
          warnings: [{ code: id, message: id, overridable: true }],
          compatibility: null
        };
        return Promise.resolve(result);
      }
    );
    const mounted = mount();
    expect(
      await screen.findByDisplayValue('Template installation-a')
    ).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByRole('button', { name: '导入应用' })).not.toHaveClass(
        'ant-btn-loading'
      )
    );
    const confirmA = modalObservation.onImport;
    expect(confirmA).toBeTypeOf('function');
    mounted.switchTo('installation-b');
    expect(
      await screen.findByDisplayValue('Template installation-b')
    ).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByRole('button', { name: '导入应用' })).not.toHaveClass(
        'ant-btn-loading'
      )
    );
    expect(api.importInstalledApplicationExtension).not.toHaveBeenCalled();
    // The TEST explicitly retains A's forwarded callback. We do not claim the
    // actual Modal retains old callbacks, or depend on a physical/microtask race.
    await act(async () => {
      confirmA!();
    });
    expect(
      api.importInstalledApplicationExtension
    ).toHaveBeenCalledExactlyOnceWith(
      'installation-a',
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
    expect(mounted.onImported).toHaveBeenCalledExactlyOnceWith(
      'server-created-id'
    );
    expect(mounted.onClose).not.toHaveBeenCalled();
    expect(
      screen.getByDisplayValue('Template installation-b')
    ).toBeInTheDocument();
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
