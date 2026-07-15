import fs from 'node:fs';
import path from 'node:path';

import { render, screen } from '@testing-library/react';
import { describe, expect, test } from 'vitest';

import { ScrollableSurface } from '../ScrollableSurface';

describe('ScrollableSurface', () => {
  test('renders a first-party scrollable surface around children', () => {
    const view = render(
      <ScrollableSurface className="custom-surface" data-testid="catalog-pane">
        <div>面板内容</div>
      </ScrollableSurface>
    );

    const surface = screen.getByTestId('catalog-pane');

    expect(surface).toHaveClass('scrollable-surface', 'custom-surface');
    expect(surface.tagName.toLowerCase()).toBe('section');
    expect(screen.getByText('面板内容')).toBeInTheDocument();
    expect(view.container.querySelector('.scrollable-surface')).toBe(surface);
  });

  test('keeps visual chrome and vertical overflow in the wrapper contract', () => {
    const cssSource = fs.readFileSync(
      path.resolve(import.meta.dirname, '../scrollable-surface.css'),
      'utf8'
    );
    const surfaceBlock = cssSource.match(
      /\.scrollable-surface\s*\{[\s\S]*?\n\}/
    )?.[0];

    expect(surfaceBlock).toContain('height: 100%;');
    expect(surfaceBlock).toContain('min-height: 0;');
    expect(surfaceBlock).toContain('overflow-y: auto;');
    expect(surfaceBlock).toContain(
      'padding: var(--scrollable-surface-padding, 16px);'
    );
    expect(surfaceBlock).toContain('border: 1px solid');
    expect(surfaceBlock).toContain(
      'border-radius: var(--scrollable-surface-radius, 12px);'
    );
  });
});
