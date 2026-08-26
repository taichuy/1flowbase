import '@testing-library/jest-dom/vitest';
import { render, screen } from '@testing-library/react';
import { describe, expect, test } from 'vitest';

import { UnrestrictedTsxBlockFrame } from '../../components/UnrestrictedTsxBlockFrame';

describe('UnrestrictedTsxBlockFrame', () => {
  test('AC-003 renders every prepared TSX Block in its own script-only iframe', () => {
    render(
      <UnrestrictedTsxBlockFrame
        blockId="block-1"
        source="export default function App() { return <div>ready</div>; }"
        style={{ width: '100%' }}
      />
    );

    const frame = screen.getByTestId(
      'frontstage-unrestricted-tsx-frame-block-1'
    );
    expect(frame).toHaveAttribute('sandbox', 'allow-scripts');
    expect(frame).toHaveAttribute(
      'srcdoc',
      expect.stringContaining('TSX Block must export')
    );
  });
});
