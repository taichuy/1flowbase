import { describe, expect, test } from 'vitest';

import { injectFrontstageContextComment } from '../../lib/jsx-studio/context-injection';

describe('Frontstage context source injection', () => {
  test('AC-005 inserts before default export and replaces a deleted/stale comment', () => {
    const source = `export default function Block({ ctx }) { return <div>{String(ctx.props.title)}</div>; }\n`;
    const comment =
      '/**\n * @1flowbase-context\n * inputs: 无\n * outputs: 无\n */';
    const injected = injectFrontstageContextComment(source, comment);
    expect(injected.indexOf(comment)).toBeLessThan(
      injected.indexOf('export default')
    );
    expect(
      injectFrontstageContextComment(injected, comment).match(
        /@1flowbase-context/g
      )
    ).toHaveLength(1);
  });
});
