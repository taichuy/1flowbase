import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { AuthenticatorUiBlockStudio } from '../components/auth-center/AuthenticatorUiBlockStudio';

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

vi.mock('../../frontstage/hooks/use-frontstage-block-catalog', () =>
  blockCatalogHook
);
vi.mock('../../frontstage/components/jsx-studio/JsxStudioResourcePanel', () => ({
  JsxStudioResourcePanel: (props: {
    configurationPanel: ReactNode;
    contextVariables?: unknown;
    runPanel?: ReactNode;
    section: string;
  }) => {
    resourcePanelHook.render(props);
    return <>{props.section === 'run' ? props.runPanel : props.configurationPanel}</>;
  }
}));
vi.mock('../../frontstage/components/JsBlockTrialPanel', () => ({
  JsBlockTrialPanel: (props: {
    block: {
      catalog: { providerCode: string; installationId: string };
      contribution: { pluginId: string; pluginVersion: string; code: string };
    };
    presentation: string;
    createRunInputs: (event?: {
      actionId: string;
      formValues?: Record<string, unknown>;
    }) => Record<string, unknown>;
  }) => {
    trialPanelHook.render(props);
    return <div>Auth Trial</div>;
  }
}));
vi.mock('@monaco-editor/react', () => ({
  default: ({
    beforeMount,
    onMount
  }: {
    beforeMount?: (monaco: unknown) => void;
    onMount?: (editor: unknown, monaco: unknown) => void;
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
    return <textarea aria-label="TSX source" />;
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
          runtimeKind: 'iframe',
          installationId: 'builtin-installation',
          providerCode: '1flowbase',
          pluginId: 'builtin-frontstage',
          pluginVersion: '1.0.0',
          contributionCode: 'frontstage.js-ui-block',
          entry: 'index.js',
          codeCapabilities: {
            monacoExtraLibs: [
              {
                source: '@1flowbase/block-sdk',
                filePath: 'file:///node_modules/@1flowbase/block-sdk/index.d.ts',
                content: "declare module '@1flowbase/block-sdk' {}"
              },
              {
                source: '@1flowbase/block-renderer/antd-facade',
                filePath:
                  'file:///node_modules/@1flowbase/block-renderer/antd-facade/index.d.ts',
                content:
                  "declare module '@1flowbase/block-renderer/antd-facade' {}"
              }
            ]
          }
        }
      ]
    });
  });

  test('AC-1444 injects the canonical block module declarations into Monaco', async () => {
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
        source="export default { main };"
        workspaceId="workspace-1"
        onClose={vi.fn()}
        onSave={vi.fn()}
      />
    );

    await waitFor(() => expect(monacoHook.addExtraLib).toHaveBeenCalledTimes(2));
    expect(monacoHook.addExtraLib).toHaveBeenNthCalledWith(
      1,
      "declare module '@1flowbase/block-sdk' {}",
      'file:///node_modules/@1flowbase/block-sdk/index.d.ts'
    );
    expect(monacoHook.addExtraLib).toHaveBeenNthCalledWith(
      2,
      "declare module '@1flowbase/block-renderer/antd-facade' {}",
      'file:///node_modules/@1flowbase/block-renderer/antd-facade/index.d.ts'
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
        source="export default { main };"
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

  test('AC-030 provides the shared resource panel with an Auth Trial run panel', () => {
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
        source="export default { main };"
        workspaceId="workspace-1"
        onClose={vi.fn()}
        onSave={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '运行预览' }));
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
      }
    });
    expect(trialProps.presentation).toBe('direct-preview');
    expect(trialProps.createRunInputs()).toEqual({
      authenticator_id: 'password-local',
      public_variables: {
        title: 'Password',
        enabled: true,
        self_registration_enabled: false
      }
    });
    expect(
      trialProps.createRunInputs({
        actionId: 'sign_up',
        formValues: { account: 'alice' }
      })
    ).toEqual({
      authenticator_id: 'password-local',
      public_variables: {
        title: 'Password',
        enabled: true,
        self_registration_enabled: false
      },
      auth_event: {
        action_id: 'sign_up',
        values: { account: 'alice' }
      }
    });
  });
});
