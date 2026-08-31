import { describe, expect, test } from 'vitest';

import { planScenarioChunk } from '../scenario-chunk-planner';
import { productAntDesignChunk } from '../native-antd-es-modules';

describe('scenario chunk planner', () => {
  test.each([
    ['/repo/node_modules/react/jsx-runtime.js', 'react-runtime'],
    ['/repo/node_modules/react-dom/client.js', 'react-runtime']
  ])('places %s in %s', (id, chunk) => {
    expect(planScenarioChunk(id)).toBe(chunk);
  });

  test.each([
    ['antd/es/button', 'antd-core'],
    ['antd/es/modal', 'antd-overlay'],
    ['antd/es/table', 'antd-data'],
    ['antd/es/result', 'antd-feedback']
  ])('classifies product source %s as %s', (source, chunk) => {
    expect(productAntDesignChunk(source)).toBe(chunk);
  });

  test('keeps application modules under route and demand-island ownership', () => {
    expect(
      planScenarioChunk('/repo/src/features/workflow/WorkflowPage.tsx')
    ).toBeUndefined();
  });

  test('does not collapse dynamic runtime inventories into eager vendors', () => {
    expect(
      planScenarioChunk(
        '\0virtual:1flowbase-native-ant-design-icon-leaf/CloseOutlined'
      )
    ).toBeUndefined();
    expect(
      planScenarioChunk('/repo/node_modules/monaco-editor/esm/editor.js')
    ).toBeUndefined();
  });
});
