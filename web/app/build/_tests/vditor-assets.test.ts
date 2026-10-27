import { expect, test, vi } from 'vitest';
import { vditorAssetsPlugin } from '../vditor-assets';

test('production emits the same local parser, rendering and font assets as development', () => {
  const plugin = vditorAssetsPlugin();
  const emitFile = vi.fn();
  const generate = plugin.generateBundle as Function;
  generate.call({ emitFile });
  const files = emitFile.mock.calls.map(([asset]) => asset.fileName);
  for (const name of [
    'index.min.js',
    'index.css',
    'js/lute/lute.min.js',
    'js/highlight.js/highlight.min.js',
    'js/katex/katex.min.js',
    'js/mermaid/mermaid.min.js',
    'css/content-theme/ant-design.css'
  ]) {
    expect(files).toContain(
      `/__1flowbase_vditor__/3.11.2/dist/${name}`.slice(1)
    );
  }
  expect(files).toContain('__1flowbase_vditor__/3.11.2/frame.html');
  expect(files.some((name) => name.endsWith('.woff2'))).toBe(true);
  expect(files.some((name) => name.includes('/echarts/'))).toBe(false);
});

test('development refuses unknown assets instead of serving an HTML fallback', () => {
  const plugin = vditorAssetsPlugin();
  const use = vi.fn();
  (plugin.configureServer as Function)({ middlewares: { use } });
  const middleware = use.mock.calls[0][0];
  const response = { statusCode: 200, end: vi.fn(), setHeader: vi.fn() };
  const next = vi.fn();
  middleware(
    { url: '/__1flowbase_vditor__/3.11.2/unknown.js' },
    response,
    next
  );
  expect(response.statusCode).toBe(404);
  expect(next).not.toHaveBeenCalled();
  middleware({ url: '/api/console/session' }, response, next);
  expect(next).toHaveBeenCalledOnce();
});
