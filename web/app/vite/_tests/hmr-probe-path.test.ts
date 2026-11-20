import { mkdtempSync, mkdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { expect, test } from 'vitest';
import { HMR_PROBE_ID, oneFlowbaseDevRuntimePlugin } from '../dev-runtime';

test('the HMR probe imports a real file inside the served app, not the private repository tmp directory', () => {
  const repo = mkdtempSync(path.join(tmpdir(), 'flowbase-hmr-path-'));
  try {
    const root = path.join(repo, 'web', 'app');
    mkdirSync(root, { recursive: true });
    const plugin = oneFlowbaseDevRuntimePlugin({
      root,
      command: 'serve',
      mode: 'test'
    });
    const load = plugin.load;
    if (!load || typeof load !== 'object')
      throw new Error('missing probe loader');
    const source = Reflect.apply(load.handler, {}, ['\0' + HMR_PROBE_ID]);
    const specifier = /import \{ generation \} from "([^"]+)"/.exec(
      String(source)
    )?.[1];
    expect(specifier).toBeTruthy();
    const file = specifier!.startsWith('/@fs/')
      ? specifier!.slice('/@fs/'.length)
      : path.join(root, specifier!);
    expect(path.relative(root, file)).not.toMatch(/^\.\.(?:[/\\]|$)/);
    expect(readFileSync(file, 'utf8')).toContain('export const generation');
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
});
