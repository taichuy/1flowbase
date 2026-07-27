import { describe, expect, test } from 'vitest';

import { blockSdkVersion, type BlockComponentProps } from '../index';

describe('Native React Block SDK contract', () => {
  test('exports the Native component props contract and module version', () => {
    const props = {
      ctx: { workspace: { id: 'workspace-1' } }
    } as BlockComponentProps;

    expect(props.ctx.workspace.id).toBe('workspace-1');
    expect(blockSdkVersion).toBe('1.0.0');
  });
});
