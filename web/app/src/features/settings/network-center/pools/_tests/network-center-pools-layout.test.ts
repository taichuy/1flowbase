import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from 'vitest';

describe('network center proxy pools layout CSS', () => {
  test('AC-001 keeps the active tab and proxy pool table on one fill-height chain', () => {
    const tabsCssSource = fs.readFileSync(
      path.resolve(
        import.meta.dirname,
        '../../../pages/network-center/network-center-section.css'
      ),
      'utf8'
    );
    const poolsCssSource = fs.readFileSync(
      path.resolve(import.meta.dirname, '../network-egress-pools.css'),
      'utf8'
    );

    expect(tabsCssSource).toMatch(
      /\.network-center-section\s*\{[^}]*display:\s*flex;[^}]*flex-direction:\s*column;[^}]*height:\s*100%;[^}]*min-height:\s*0;[^}]*\}/s
    );
    expect(tabsCssSource).toMatch(
      /\.network-center-section\s*>\s*\.ant-tabs-body-holder[^{]*\{[^}]*display:\s*flex;[^}]*flex:\s*1 1 auto;[^}]*min-height:\s*0;[^}]*overflow:\s*hidden;[^}]*\}/s
    );
    expect(tabsCssSource).toMatch(
      /\.network-center-section\s+\.ant-tabs-content\s*>\s*\*\s*\{[^}]*height:\s*100%;[^}]*min-height:\s*0;[^}]*\}/s
    );
    expect(poolsCssSource).toMatch(
      /\.network-center-pools-shell\s*\{[^}]*display:\s*flex;[^}]*flex-direction:\s*column;[^}]*height:\s*100%;[^}]*min-height:\s*0;[^}]*\}/s
    );
  });
});
