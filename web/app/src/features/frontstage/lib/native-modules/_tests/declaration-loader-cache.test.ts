import { afterEach, describe, expect, test, vi } from 'vitest';

import { createNativeDeclarationLoader } from '../editor-declarations/loader';

describe('native declaration loader HTTP cache policy', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  test('uses force-cache for content-addressed /assets/native-declarations/<sha256>.json URLs so reload never revalidates', async () => {
    const hashA = 'a'.repeat(64);
    const hashB = 'b'.repeat(64);
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockImplementation(async (input) => {
        const url = String(input);
        return new Response(
          JSON.stringify([
            {
              source: url,
              content: `declare module '${url}';`,
              filePath: `file:///node_modules/${url}/index.d.ts`
            }
          ]),
          {
            status: 200,
            headers: { 'Content-Type': 'application/json' }
          }
        );
      });

    const loader = createNativeDeclarationLoader(
      {
        version: 1,
        roots: {
          react: ['r'],
          antd: ['nested'],
          custom: ['dev']
        },
        resources: {
          r: `/assets/native-declarations/${hashA}.json`,
          nested: `/nested/assets/native-declarations/${hashB}.json`,
          dev: '/custom/declarations.json'
        }
      },
      []
    );

    await loader.loadForSource(`import 'antd'; import 'custom';`);

    expect(fetchSpy).toHaveBeenCalledTimes(3);
    expect(fetchSpy).toHaveBeenCalledWith(
      `/assets/native-declarations/${hashA}.json`,
      { cache: 'force-cache' }
    );
    expect(fetchSpy).toHaveBeenCalledWith(
      `/nested/assets/native-declarations/${hashB}.json`,
      { cache: 'force-cache' }
    );
    expect(fetchSpy).toHaveBeenCalledWith('/custom/declarations.json');
  });
});
