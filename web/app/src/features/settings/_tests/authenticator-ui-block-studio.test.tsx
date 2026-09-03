import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeAll, beforeEach, describe, expect, test, vi } from 'vitest';

import { LoginEntryUiBlockStudio } from '../components/auth-center/LoginEntryUiBlockStudio';
import { LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC } from '@1flowbase/page-runtime';
import { loadApplicationI18nResources } from '../../../shared/i18n/app-i18n';

const blockCatalogHook = vi.hoisted(() => ({
  useFrontstageBlockCatalog: vi.fn()
}));
const monacoHook = vi.hoisted(() => ({
  addExtraLib: vi.fn(),
  setCompilerOptions: vi.fn()
}));
const resourcePanelHook = vi.hoisted(() => ({
  render: vi.fn()
}));
const trialPanelHook = vi.hoisted(() => ({
  render: vi.fn()
}));

vi.mock(
  '../../frontstage/hooks/use-frontstage-block-catalog',
  () => blockCatalogHook
);
vi.mock('../../../shared/code-block/monaco-runtime', () => ({
  loadMonacoEditorModule: () => import('@monaco-editor/react')
}));
vi.mock(
  '../../frontstage/components/jsx-studio/JsxStudioResourcePanel',
  () => ({
    JsxStudioResourcePanel: (props: {
      configurationPanel: ReactNode;
      contextVariables?: unknown;
      runPanel?: ReactNode;
      section: string;
    }) => {
      resourcePanelHook.render(props);
      return (
        <>
          {props.section === 'run'
            ? props.runPanel
            : props.section === 'configuration'
              ? props.configurationPanel
              : null}
        </>
      );
    }
  })
);
vi.mock('../../frontstage/components/jsx-studio/JsxStudioRunPanel', () => ({
  JsxStudioRunPanel: (props: {
    block: {
      catalog: { providerCode: string; installationId: string };
      contribution: { pluginId: string; pluginVersion: string; code: string };
    };
    code: string;
    revision: string;
    createBlockContext: (input: {
      requestId: string;
      instanceEpoch: string;
      plan: Record<string, unknown>;
      isCurrentInstance(): boolean;
      observeApiCall(observation: Record<string, unknown>): void;
    }) => { inputs: Record<string, unknown>; application: unknown };
  }) => {
    trialPanelHook.render(props);
    return <div>Auth Studio Run</div>;
  }
}));
vi.mock('@monaco-editor/react', () => ({
  default: ({
    beforeMount,
    onChange,
    onMount,
    value
  }: {
    beforeMount?: (monaco: unknown) => void;
    onChange?: (value: string) => void;
    onMount?: (editor: unknown, monaco: unknown) => void;
    value?: string;
  }) => {
    const monaco = {
      MarkerSeverity: { Error: 8 },
      editor: { setModelMarkers: vi.fn() },
      languages: {
        typescript: {
          JsxEmit: { Preserve: 'preserve', ReactJSX: 'react-jsx' },
          ModuleResolutionKind: { NodeJs: 'node-js' },
          ScriptTarget: { ES2022: 'es2022' },
          typescriptDefaults: {
            addExtraLib: monacoHook.addExtraLib,
            setCompilerOptions: monacoHook.setCompilerOptions
          }
        }
      }
    };
    const editor = {
      getModel: () => ({
        uri: { toString: () => 'file:///auth-center/password-local.tsx' }
      })
    };
    beforeMount?.(monaco);
    onMount?.(editor, monaco);
    return (
      <textarea
        aria-label="TSX source"
        value={value}
        onChange={(event) => onChange?.(event.target.value)}
      />
    );
  }
}));

describe('LoginEntryUiBlockStudio', () => {
  beforeAll(async () => {
    await loadApplicationI18nResources();
  });

  beforeEach(() => {
    vi.clearAllMocks();
    monacoHook.addExtraLib.mockReturnValue({ dispose: vi.fn() });
    Object.defineProperty(window, 'innerWidth', {
      configurable: true,
      value: 1400
    });
    Object.defineProperty(window, 'innerHeight', {
      configurable: true,
      value: 900
    });
    blockCatalogHook.useFrontstageBlockCatalog.mockReturnValue({
      items: [
        {
          id: '1flowbase:frontstage.js-ui-block',
          runtimeKind: 'native_react',
          installationId: 'builtin-installation',
          providerCode: '1flowbase',
          pluginId: 'builtin-frontstage',
          pluginVersion: '1.0.0',
          contributionCode: 'frontstage.js-ui-block',
          entry: 'index.js',
          codeModules: []
        }
      ]
    });
  });

  test('D4-AC-005 injects the standard Native React declarations into Monaco', async () => {
    render(
      <LoginEntryUiBlockStudio
        loginEntryId="password-local"
        loginEntryTitle="Password"
        authType="password_local"
        contextVariables={[
          {
            label: 'ctx.inputs.login_entry_id',
            member_path: 'inputs.login_entry_id',
            schema: { type: 'string' }
          }
        ]}
        description={null}
        enabled
        errorMessage={null}
        interfacePathPrefixes={['/api/public/']}
        publicVariables={{
          title: 'Password',
          enabled: true,
          self_registration_enabled: true
        }}
        open
        readOnly={false}
        saving={false}
        selfRegistrationEnabled
        source="export default function AuthBlock({ ctx }) { return <div>{String(ctx.props.title)}</div>; }"
        workspaceId="workspace-1"
        onClose={vi.fn()}
        onSave={vi.fn()}
      />
    );

    await waitFor(
      () =>
        expect(screen.getByRole('group', { name: '代码' })).toHaveAttribute(
          'aria-busy',
          'false'
        ),
      { timeout: 5000 }
    );
    await waitFor(
      () =>
        expect(monacoHook.addExtraLib).toHaveBeenCalledWith(
          expect.stringContaining('declare namespace JSX'),
          'file:///1flowbase/native-react-jsx.d.ts'
        ),
      { timeout: 5000 }
    );
    expect(monacoHook.addExtraLib).toHaveBeenCalledWith(
      expect.stringContaining('interface NativeReactBlockContext'),
      'file:///1flowbase/native-react-context.d.ts'
    );
    expect(monacoHook.addExtraLib).toHaveBeenCalledWith(
      expect.stringContaining("declare module '@1flowbase/block-sdk'"),
      'file:///node_modules/@1flowbase/block-sdk/index.d.ts'
    );
    fireEvent.click(screen.getByRole('button', { name: '变量' }));
    expect(resourcePanelHook.render).toHaveBeenCalledWith(
      expect.objectContaining({
        contextVariables: [
          expect.objectContaining({ member_path: 'inputs.login_entry_id' })
        ]
      })
    );
  });

  test('AC-024 marks a missing Auth context catalog unavailable', async () => {
    render(
      <LoginEntryUiBlockStudio
        loginEntryId="password-local"
        loginEntryTitle="Password"
        authType="password_local"
        contextVariables={undefined as never}
        description={null}
        enabled
        errorMessage={null}
        interfacePathPrefixes={['/api/public/']}
        publicVariables={{
          title: 'Password',
          enabled: true,
          self_registration_enabled: true
        }}
        open
        readOnly={false}
        saving={false}
        selfRegistrationEnabled
        source="export default function AuthBlock() { return null; }"
        workspaceId="workspace-1"
        onClose={vi.fn()}
        onSave={vi.fn()}
      />
    );

    fireEvent.click(await screen.findByRole('button', { name: '变量' }));
    expect(resourcePanelHook.render).toHaveBeenCalledWith(
      expect.objectContaining({ contextVariables: undefined })
    );
  });

  test('AC-004 uses the shared Studio configuration panel', async () => {
    render(
      <LoginEntryUiBlockStudio
        loginEntryId="password-local"
        loginEntryTitle="Password"
        authType="password_local"
        contextVariables={[]}
        description={null}
        enabled
        errorMessage={null}
        interfacePathPrefixes={['/api/public/']}
        publicVariables={{ title: 'Password', enabled: true }}
        open
        readOnly={false}
        saving={false}
        selfRegistrationEnabled={false}
        source="export default function AuthBlock() { return null; }"
        workspaceId="workspace-1"
        onClose={vi.fn()}
        onSave={vi.fn()}
      />
    );

    fireEvent.click(await screen.findByRole('button', { name: '区块设置' }));
    expect(
      screen
        .getByText('Password')
        .closest('.frontstage-jsx-studio__configuration-panel')
    ).not.toBeNull();
  });

  test('AC-043/044/045 runs the current draft from the header without saving it', async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <LoginEntryUiBlockStudio
        loginEntryId="password-local"
        loginEntryTitle="Password"
        authType="password_local"
        contextVariables={[]}
        description={null}
        enabled
        errorMessage={null}
        interfacePathPrefixes={['/api/public/']}
        publicVariables={{
          title: 'Password',
          enabled: true,
          self_registration_enabled: false
        }}
        open
        readOnly={false}
        saving={false}
        selfRegistrationEnabled={false}
        source="export default function AuthBlock() { return null; }"
        workspaceId="workspace-1"
        onClose={vi.fn()}
        onSave={onSave}
      />
    );

    fireEvent.change(
      await screen.findByRole(
        'textbox',
        { name: 'TSX source' },
        { timeout: 5000 }
      ),
      {
        target: { value: 'first unsaved draft' }
      }
    );
    fireEvent.click(screen.getByRole('button', { name: /^运\s*行$/ }));
    expect(resourcePanelHook.render).toHaveBeenLastCalledWith(
      expect.objectContaining({ runPanel: expect.anything() })
    );
    expect(screen.getByText('Auth Studio Run')).toBeInTheDocument();
    const trialProps = trialPanelHook.render.mock.calls.at(-1)?.[0];
    expect(trialProps.block).toMatchObject({
      catalog: {
        providerCode: 'public-auth',
        installationId: 'public-auth-authoring'
      },
      contribution: {
        pluginId: 'public-auth',
        pluginVersion: '1.0.0',
        code: 'authenticator-ui'
      },
      runtime: {
        kind: 'native_trusted_block',
        hint: 'native_trusted_block'
      }
    });
    expect(trialProps.code).toBe('first unsaved draft');
    expect(trialProps.revision).toEqual(expect.any(String));
    const firstRevision = trialProps.revision;
    expect(onSave).not.toHaveBeenCalled();

    fireEvent.change(screen.getByRole('textbox', { name: 'TSX source' }), {
      target: { value: 'second unsaved draft' }
    });
    expect(trialPanelHook.render.mock.calls.at(-1)?.[0]).toEqual(
      expect.objectContaining({
        code: 'first unsaved draft',
        revision: firstRevision
      })
    );

    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));
    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith('second unsaved draft')
    );
    expect(trialPanelHook.render.mock.calls.at(-1)?.[0]).toEqual(
      expect.objectContaining({
        code: 'first unsaved draft',
        revision: firstRevision
      })
    );
    const observeApiCall = vi.fn();
    const previewContext = trialProps.createBlockContext({
      requestId: 'draft:public-auth:password-local:1',
      instanceEpoch: 'auth-preview-1',
      plan: {
        runtime: 'native_trusted_block',
        blockId: 'public-auth:password-local',
        entry: 'default',
        source: 'first unsaved draft',
        normalizedSource: 'first unsaved draft',
        props: {},
        requiredPermissions: ['ui_block.javascript.native']
      },
      isCurrentInstance: () => true,
      observeApiCall
    });
    expect(previewContext.inputs).toEqual({
      login_entry_id: 'password-local',
      authenticator_selection_available: false,
      public_variables: {
        title: 'Password',
        enabled: true,
        self_registration_enabled: false
      }
    });
    expect(previewContext.application).toBeNull();
  });

  test('AC-013 keeps editor errors in a content-sized notice row', async () => {
    render(
      <LoginEntryUiBlockStudio
        loginEntryId="password-local"
        loginEntryTitle="Password"
        authType="password_local"
        contextVariables={[]}
        description={null}
        enabled
        errorMessage="invalid input: public_ui_block"
        interfacePathPrefixes={['/api/public/']}
        publicVariables={{ title: 'Password', enabled: true }}
        open
        readOnly={false}
        saving={false}
        selfRegistrationEnabled={false}
        source="export default function AuthBlock() { return null; }"
        workspaceId="workspace-1"
        onClose={vi.fn()}
        onSave={vi.fn()}
      />
    );

    expect((await screen.findByRole('alert')).parentElement).toHaveClass(
      'frontstage-jsx-studio__editor-notice'
    );
  });

  test('AC-002 keeps legacy source blocked and restores the registered default on confirmation', async () => {
    const legacySource = `import { Form } from '@1flowbase/block-renderer/antd-facade';
async function main(ctx) { return { view: <Form />, outputs: {} }; }
export default { main } satisfies BlockModule;`;
    const defaultSource =
      'export default function PasswordLocalAuth() { return null; }';
    const onSave = vi.fn();
    render(
      <LoginEntryUiBlockStudio
        loginEntryId="password-local"
        loginEntryTitle="Password"
        authType="password_local"
        contextVariables={[]}
        defaultSource={defaultSource}
        description={null}
        enabled
        errorMessage={null}
        interfacePathPrefixes={['/api/public/']}
        publicVariables={{ self_registration_enabled: true }}
        open
        readOnly={false}
        saving={false}
        selfRegistrationEnabled
        source={legacySource}
        workspaceId="workspace-1"
        onClose={vi.fn()}
        onSave={onSave}
      />
    );

    expect(
      await screen.findByText(LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC.message)
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /运\s*行/ }));
    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));
    expect(onSave).not.toHaveBeenCalled();
    expect(trialPanelHook.render).not.toHaveBeenCalled();

    fireEvent.click(
      screen.getByRole('button', {
        name: '恢复默认'
      })
    );
    fireEvent.click(await screen.findByRole('button', { name: /确\s*认/ }));
    await waitFor(() => expect(onSave).toHaveBeenCalledWith(defaultSource));
  });
});
