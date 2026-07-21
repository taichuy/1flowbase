import { describe, expect, test } from 'vitest';

import {
  createFrontstageContextComment,
  createFrontstageJsxEditorProjection
} from '../../lib/jsx-studio/editor-projection';

describe('Frontstage JSX editor projection', () => {
  test('AC-021 projects only catalog components and the editable context comment', () => {
    const projection = createFrontstageJsxEditorProjection({
      catalogEntry: null
    });

    expect(projection).toEqual({
      components: [],
      contextComment: createFrontstageContextComment(),
      monacoExtraLibs: []
    });
    expect(projection.contextComment).toContain('@1flowbase-context');
    expect(projection.contextComment).not.toContain('interfaces:');
    expect(projection.contextComment).not.toContain('ctx.data');
  });
});
