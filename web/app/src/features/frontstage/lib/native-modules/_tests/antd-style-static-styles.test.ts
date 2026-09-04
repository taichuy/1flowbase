import { describe, expect, test } from 'vitest';

import {
  compileNativeReactComponent,
  evaluateNativeReactComponentArtifactWithRegistry,
  type NativeReactResolvedModuleAsset
} from '@1flowbase/page-runtime';

import { createFrontstageNativeReactModuleRegistry } from '../registry';

describe('antd-style static style artifacts', () => {
  test('I1989-AC-static-style turns module-top-level createStaticStyles output into artifact-local shadow styles', async () => {
    const first = await evaluateStaticStyleArtifact('#123456');
    const second = await evaluateStaticStyleArtifact('#654321');

    expect(first.beforeEvaluation).toEqual([]);
    expect(first.asset).toMatchObject({
      module_source: 'antd-style',
      role: 'shadow_style',
      media_type: 'text/css; charset=utf-8'
    });
    expect(first.className).not.toBe('');
    expect(first.css).toContain(first.className);
    expect(first.css).toContain('#123456');
    expect(first.css).not.toContain('#654321');
    expect(second.css).toContain(second.className);
    expect(second.css).toContain('#654321');
    expect(second.css).not.toContain('#123456');
    expect(document.head).not.toHaveTextContent('#123456');
    expect(document.head).not.toHaveTextContent('#654321');
  });
});

async function evaluateStaticStyleArtifact(color: string): Promise<{
  beforeEvaluation: NativeReactResolvedModuleAsset[];
  asset: NativeReactResolvedModuleAsset | undefined;
  className: string;
  css: string;
}> {
  const registry = createFrontstageNativeReactModuleRegistry();
  await registry.load('antd-style');
  const beforeEvaluation = await registry.resolveModuleAssets(['antd-style']);
  const compiled = compileNativeReactComponent(
    `import { createStaticStyles } from 'antd-style';
const styles = createStaticStyles(({ css }) => ({ root: css({ color: '${color}' }) }));
export default function Block() { return <div className={styles.root} />; }`,
    registry.definitions
  );
  if (!compiled.ok) throw new Error('Static style fixture failed to compile.');

  const evaluated = await evaluateNativeReactComponentArtifactWithRegistry(
    compiled.artifact,
    registry
  );
  if (!evaluated.ok)
    throw new Error('Static style fixture failed to evaluate.');
  const element = evaluated.component({} as never) as unknown as {
    props: { className: string };
  };
  const assets = await registry.resolveModuleAssets(['antd-style']);

  return {
    beforeEvaluation,
    asset: assets[0],
    className: element.props.className,
    css: new TextDecoder().decode(assets[0]?.bytes)
  };
}
