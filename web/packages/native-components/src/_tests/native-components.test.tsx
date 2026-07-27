import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { ScrollableSurface, Surface } from '../index';

describe('@1flowbase/native-components (AC-PUB-001)', () => {
  it('forwards safe DOM props while retaining official surface classes', () => {
    render(
      <Surface as="article" aria-label="surface" data-kind="fixture">
        content
      </Surface>
    );

    const surface = screen.getByLabelText('surface');
    expect(surface.tagName).toBe('ARTICLE');
    expect(surface).toHaveClass('oneflow-surface');
    expect(surface).toHaveAttribute('data-kind', 'fixture');
  });

  it('provides the bounded scroll surface without exposing lifecycle state', () => {
    render(<ScrollableSurface aria-label="scroll">content</ScrollableSurface>);
    expect(screen.getByLabelText('scroll')).toHaveClass(
      'oneflow-surface',
      'oneflow-scrollable-surface'
    );
  });
});
// @vitest-environment jsdom
