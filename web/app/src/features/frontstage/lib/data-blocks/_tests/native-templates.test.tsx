import { readFileSync } from 'node:fs';
import { cleanup, render, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  compileNativeReactComponent,
  evaluateNativeReactComponentArtifactWithRegistry,
  type NativeTrustedBlockPreparePlan
} from '@1flowbase/page-runtime';
import { i18nText } from '../../../../../shared/i18n/text';
import {
  FrontstageNativeTrustedBlockPortalHost,
  type FrontstageNativeTrustedBlockReactComponent
} from '../../native-adapter/native-trusted-block-react-adapter';
import { createFrontstageNativeReactModuleRegistry } from '../../native-modules/registry';
import { createFrontstageUnavailableBlockContext } from '../../native-trusted-block/unavailable-context';

const roots: HTMLElement[] = [];
afterEach(() => {
  cleanup();
  for (const root of roots.splice(0)) root.remove();
});

/** Compile the actual shipped YAML scalar rather than duplicating template source. */
function contributionTemplate(code: string): string {
  const manifest = readFileSync(
    new URL(
      '../../../../../../../../api/plugins/capability-plugins/1flowbase/manifest.yaml',
      import.meta.url
    ),
    'utf8'
  );
  const entry = manifest
    .split(`  - contribution_code: ${code}\n`)[1]
    ?.split('\n  - contribution_code:')[0];
  const source = entry?.match(
    /^    code_template: \|\r?\n([\s\S]*?)(?=^    [A-Za-z_])/m
  )?.[1];
  if (!source) throw new Error(`Missing native template: ${code}`);
  return source.replace(/^ {6}/gm, '').trim();
}

function surfaceCss(root: ShadowRoot): string {
  return [
    ...Array.from(
      root.querySelectorAll('style'),
      (style) => style.textContent ?? ''
    ),
    ...Array.from(root.adoptedStyleSheets ?? [], (sheet) =>
      Array.from(sheet.cssRules, (rule) => rule.cssText).join('\n')
    )
  ].join('\n');
}

describe('shipped data block native templates', () => {
  it.each([
    [
      'frontstage.table-block',
      'TableBlock',
      'data_blocks.table.invalid_config'
    ],
    ['frontstage.form-block', 'FormBlock', 'data_blocks.form.not_configured']
  ] as const)(
    'compiles and renders %s with real registered components and surface-owned styles',
    async (contributionCode, exportName, errorKey) => {
      const registry = createFrontstageNativeReactModuleRegistry();
      expect(
        registry.definitions.find(
          (entry) => entry.module_source === '@1flowbase/data-blocks'
        )?.exports
      ).toEqual(['TableBlock', 'FormBlock']);
      const source = contributionTemplate(contributionCode);
      const compiled = compileNativeReactComponent(
        source,
        registry.definitions
      );
      expect(compiled.diagnostics).toEqual([]);
      if (!compiled.ok)
        throw new Error(`Template compilation failed: ${contributionCode}`);
      const moduleSources = compiled.artifact.program.injectedModules.map(
        (module) => module.source
      );
      expect(moduleSources).toContain('@1flowbase/data-blocks');
      const evaluated = await evaluateNativeReactComponentArtifactWithRegistry(
        compiled.artifact,
        registry
      );
      expect(evaluated.diagnostics).toEqual([]);
      if (!evaluated.ok)
        throw new Error(`Template evaluation failed: ${contributionCode}`);
      const module = await registry.load('@1flowbase/data-blocks');
      expect(module[exportName]).toBeTypeOf('function');
      const moduleAssets = await registry.resolveModuleAssets(moduleSources);
      const blockStyles = moduleAssets.filter(
        (asset) =>
          asset.module_source === '@1flowbase/data-blocks' &&
          asset.role === 'shadow_style'
      );
      expect(blockStyles).toHaveLength(2);
      const loadedCss = blockStyles
        .map((asset) => new TextDecoder().decode(asset.bytes))
        .join('\n');
      expect(loadedCss).toContain('.data-table-block');
      expect(loadedCss).toContain('.data-form-block');

      const plan: NativeTrustedBlockPreparePlan = {
        runtime: 'native_trusted_block',
        blockId: `smoke-${contributionCode}`,
        entry: 'default',
        source,
        normalizedSource: source,
        props: {},
        requiredPermissions: ['ui_block.javascript.native']
      };
      const root = document.createElement('div');
      document.body.append(root);
      roots.push(root);
      const onRuntimeError = vi.fn();
      const ctx = createFrontstageUnavailableBlockContext(plan);
      const get = vi.spyOn(ctx.api, 'get');
      render(
        <FrontstageNativeTrustedBlockPortalHost
          root={root}
          renderEpoch={contributionCode}
          plan={plan}
          component={
            evaluated.component as FrontstageNativeTrustedBlockReactComponent
          }
          ctx={ctx}
          moduleAssets={moduleAssets}
          onRuntimeError={onRuntimeError}
        />
      );
      await waitFor(() => expect(root.shadowRoot).not.toBeNull());
      const shadowRoot = root.shadowRoot!;
      const alert = await within(
        shadowRoot as unknown as HTMLElement
      ).findByRole('alert');
      expect(alert).toHaveTextContent(i18nText('frontstage', errorKey));
      expect(onRuntimeError).not.toHaveBeenCalled();
      expect(get).not.toHaveBeenCalled();
      await waitFor(() => {
        expect(surfaceCss(shadowRoot)).toContain('.data-table-block');
        expect(surfaceCss(shadowRoot)).toContain('.data-form-block');
      });
      const globalCss = Array.from(
        document.head.querySelectorAll('style'),
        (style) => style.textContent ?? ''
      ).join('\n');
      expect(globalCss).not.toContain('.data-table-block');
      expect(globalCss).not.toContain('.data-form-block');
    }
  );
});
