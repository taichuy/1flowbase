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

vi.mock('../../frontstage/hooks/use-frontstage-block-catalog', () =>
  blockCatalogHook
);
vi.mock('../../frontstage/components/jsx-studio/JsxStudioResourcePanel', () => ({
  JsxStudioResourcePanel: (props: {
    configurationPanel: ReactNode;
    contextVariables?: unknown;
  }) => {
    resourcePanelHook.render(props);
    return <>{props.configurationPanel}</>;
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
          providerCode: '1flowbase',
          contributionCode: 'frontstage.js-ui-block',
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
});
