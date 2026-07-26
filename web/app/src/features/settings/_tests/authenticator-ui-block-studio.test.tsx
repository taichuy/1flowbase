import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { AuthenticatorUiBlockStudio } from '../components/auth-center/AuthenticatorUiBlockStudio';
import { LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC } from '@1flowbase/page-runtime';

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
vi.mock('../../frontstage/components/JsBlockTrialPanel', () => ({
  JsBlockTrialPanel: (props: {
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
    }) => { inputs: Record<string, unknown>; application: unknown };
  }) => {
    trialPanelHook.render(props);
    return <div>Auth Trial</div>;
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
    beforeMount?.(monaco);
    onMount?.({}, monaco);
    return (
      <textarea
        aria-label="TSX source"
        value={value}
        onChange={(event) => onChange?.(event.target.value)}
      />
    );
  }
}));

describe('AuthenticatorUiBlockStudio', () => {
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
      <AuthenticatorUiBlockStudio
        authenticatorId="password-local"
        authenticatorTitle="Password"
        authType="password_local"
        contextVariables={[
          {
            label: 'ctx.inputs.authenticator_id',
            member_path: 'inputs.authenticator_id',
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

    await waitFor(() =>
      expect(monacoHook.addExtraLib).toHaveBeenCalledTimes(2)
    );
    expect(monacoHook.addExtraLib).toHaveBeenNthCalledWith(
      1,
      expect.stringContaining("declare module 'react'"),
      'file:///node_modules/@types/react/index.d.ts'
    );
    expect(monacoHook.addExtraLib).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining('interface NativeReactBlockContext'),
      'file:///1flowbase/native-react-context.d.ts'
    );
    expect(resourcePanelHook.render).toHaveBeenCalledWith(
      expect.objectContaining({
        contextVariables: [
          expect.objectContaining({ member_path: 'inputs.authenticator_id' })
        ]
      })
    );
  });

  test('AC-024 marks a missing Auth context catalog unavailable', () => {
    render(
      <AuthenticatorUiBlockStudio
        authenticatorId="password-local"
        authenticatorTitle="Password"
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

    fireEvent.click(screen.getByRole('button', { name: '变量' }));
    expect(resourcePanelHook.render).toHaveBeenCalledWith(
      expect.objectContaining({ contextVariables: null })
    );
  });

  test('AC-004 uses the shared Studio configuration panel', () => {
    render(
      <AuthenticatorUiBlockStudio
        authenticatorId="password-local"
        authenticatorTitle="Password"
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

    fireEvent.click(screen.getByRole('button', { name: '区块设置' }));
    expect(
      screen
        .getByText('Password')
        .closest('.frontstage-jsx-studio__configuration-panel')
    ).not.toBeNull();
  });

  test('AC-043/044/045 runs the current draft from the header without saving it', async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <AuthenticatorUiBlockStudio
        authenticatorId="password-local"
        authenticatorTitle="Password"
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

    fireEvent.change(screen.getByRole('textbox', { name: 'TSX source' }), {
      target: { value: 'first unsaved draft' }
    });
    fireEvent.click(screen.getByRole('button', { name: /^运\s*行$/ }));
    expect(resourcePanelHook.render).toHaveBeenLastCalledWith(
      expect.objectContaining({ runPanel: expect.anything() })
    );
    expect(screen.getByText('Auth Trial')).toBeInTheDocument();
    const trialProps = trialPanelHook.render.mock.calls.at(-1)?.[0];
    expect(trialProps.block).toMatchObject({
      catalog: {
        providerCode: '1flowbase',
        installationId: 'builtin-installation'
      },
      contribution: {
        pluginId: 'builtin-frontstage',
        pluginVersion: '1.0.0',
        code: 'frontstage.js-ui-block'
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
      isCurrentInstance: () => true
    });
    expect(previewContext.inputs).toEqual({
      authenticator_id: 'password-local',
      public_variables: {
        title: 'Password',
        enabled: true,
        self_registration_enabled: false
      }
    });
    expect(previewContext.application).toBeNull();
  });

  test('AC-013 keeps editor errors in a content-sized notice row', () => {
    render(
      <AuthenticatorUiBlockStudio
        authenticatorId="password-local"
        authenticatorTitle="Password"
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

    expect(screen.getByRole('alert').parentElement).toHaveClass(
      'frontstage-jsx-studio__editor-notice'
    );
  });

  test('D4-AC-006 does not save, preview, or rewrite a controlled legacy Auth source', () => {
    const legacySource = `import { Form } from '@1flowbase/block-renderer/antd-facade';
async function main(ctx) { return { view: <Form />, outputs: {} }; }
export default { main } satisfies BlockModule;`;
    const onSave = vi.fn();
    render(
      <AuthenticatorUiBlockStudio
        authenticatorId="password-local"
        authenticatorTitle="Password"
        authType="password_local"
        contextVariables={[]}
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

    expect(screen.getByRole('textbox', { name: 'TSX source' })).toHaveValue(
      legacySource
    );
    expect(
      screen.getByText(LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC.message)
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /^运\s*行$/ }));
    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));
    expect(onSave).not.toHaveBeenCalled();
    expect(trialPanelHook.render).not.toHaveBeenCalled();
  });
});
