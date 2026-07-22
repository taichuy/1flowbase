import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import type { FrontstagePageContent } from '../api/page-content';
import { PageCanvas } from '../components/PageCanvas';
import {
  createFrontstagePageContentFixture,
  type FrontstagePageContentFixtureOverrides
} from './frontstage-page-content-fixtures';

function createPageContent(
  overrides: FrontstagePageContentFixtureOverrides = {}
): FrontstagePageContent {
  return createFrontstagePageContentFixture(overrides);
}

describe('PageCanvas', () => {
  test('renders a compact loading state before content is available', () => {
    render(<PageCanvas isLoading />);

    expect(screen.getByText('页面内容加载中')).toBeInTheDocument();
    expect(
      screen.getByText('正在读取页面内容和区块清单。')
    ).toBeInTheDocument();
  });

  test('renders an error state with retry action', () => {
    const onRetry = vi.fn();

    render(<PageCanvas hasError onRetry={onRetry} />);

    expect(screen.getByText('页面内容加载失败')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /重\s*试/ }));

    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  test('renders an unselected empty state without content', () => {
    const { container } = render(<PageCanvas content={undefined} />);

    expect(container.querySelector('.ant-empty')).toBeInTheDocument();
    expect(screen.queryByText('未选择页面内容')).not.toBeInTheDocument();
    expect(
      screen.queryByText('选择页面后将显示页面预览。')
    ).not.toBeInTheDocument();
  });

  test('renders page title and an empty illustration', () => {
    const { container } = render(<PageCanvas content={createPageContent()} />);

    expect(screen.getByText('Landing')).toBeInTheDocument();
    expect(container.querySelector('.ant-empty')).toBeInTheDocument();
  });

  test('#1300 renders a compact design empty state instead of a block-like Ant Empty surface', () => {
    const { container } = render(
      <PageCanvas content={createPageContent()} isDesignMode />
    );

    expect(
      screen.getByTestId('page-canvas-design-empty-state')
    ).toBeEmptyDOMElement();
    expect(container.querySelector('.ant-empty')).not.toBeInTheDocument();
    expect(
      container.querySelector('[style*="dashed"]')
    ).not.toBeInTheDocument();
  });

  test('AC-012 keeps the measured canvas host mounted when the first block is created', () => {
    const view = render(
      <PageCanvas content={createPageContent()} isDesignMode />
    );
    const measuredHost = screen.getByTestId('page-canvas-render-slots');

    view.rerender(
      <PageCanvas
        content={createPageContent({
          root: {
            uid: 'root-1',
            payload: {
              blocks: [
                {
                  id: 'first-block',
                  renderer_version: 'v1',
                  codeRef: 'first-block-code',
                  contributionCode: 'official.first-block',
                  runtime: 'inline'
                }
              ]
            }
          }
        })}
        isDesignMode
      />
    );

    expect(screen.getByTestId('page-canvas-render-slots')).toBe(measuredHost);
    expect(
      within(measuredHost).getByTestId('block-slot-first-block')
    ).toBeInTheDocument();
  });

  test('renders blocks sorted by order — each block shows loading placeholder', () => {
    render(
      <PageCanvas
        content={createPageContent({
          root: {
            uid: 'root-1',
            payload: {
              blocks: [
                {
                  id: 'hero',
                  renderer_version: 'v1',
                  codeRef: 'hero-code',
                  contributionCode: 'official.hero',
                  runtime: { kind: 'iframe', entry: 'blocks/hero.html' },
                  layout: { order: 20, region: 'main' }
                },
                {
                  id: 'cta',
                  renderer_version: 'v1',
                  codeRef: 'cta-code',
                  contributionCode: 'official.cta',
                  runtime: 'inline',
                  layout: { order: 10, region: 'footer' }
                }
              ]
            }
          }
        })}
      />
    );

    expect(
      within(screen.getByTestId('page-canvas-render-slots')).getAllByTestId(
        'block-ui-loading-shell'
      )
    ).toHaveLength(2);
    expect(screen.queryByText('区块加载中...')).not.toBeInTheDocument();
  });

  test('shows loading placeholder for blocks without runtime sessions', () => {
    render(
      <PageCanvas
        content={createPageContent({
          root: {
            uid: 'root-1',
            payload: {
              blocks: [
                {
                  id: 'hero',
                  renderer_version: 'v1',
                  codeRef: 'hero-code',
                  contributionCode: 'official.hero',
                  runtime: { kind: 'iframe', entry: 'blocks/hero.js' },
                  layout: { order: 10, region: 'main', span: 12 }
                }
              ]
            }
          }
        })}
      />
    );

    const slots = within(screen.getByTestId('page-canvas-render-slots'));
    expect(slots.getByTestId('block-ui-loading-shell')).toHaveAttribute(
      'aria-busy',
      'true'
    );
    expect(slots.queryByText('区块加载中...')).not.toBeInTheDocument();
  });

  test('shows an explicit error instead of rendering an unsupported renderer version', () => {
    render(
      <PageCanvas
        content={createPageContent({
          root: {
            uid: 'root-1',
            payload: {
              blocks: [
                {
                  id: 'future',
                  renderer_version: 'v2',
                  codeRef: 'future-code',
                  contributionCode: 'official.future',
                  runtime: { kind: 'iframe', entry: 'blocks/future.js' }
                }
              ]
            }
          }
        })}
      />
    );

    expect(screen.getByText('区块渲染版本不受支持')).toBeInTheDocument();
  });

  test('notifies selection changes when clicked in design mode', () => {
    const onSelectBlock = vi.fn();

    render(
      <PageCanvas
        onSelectBlock={onSelectBlock}
        content={createPageContent({
          root: {
            uid: 'root-1',
            payload: {
              blocks: [
                {
                  id: 'hero',
                  renderer_version: 'v1',
                  codeRef: 'hero-code',
                  contributionCode: 'official.hero',
                  runtime: 'inline'
                }
              ]
            }
          }
        })}
        isDesignMode
      />
    );

    // In design mode, block containers have role="button"
    const slots = within(screen.getByTestId('page-canvas-render-slots'));

    fireEvent.click(slots.getByRole('button', { name: '区块 hero' }));

    expect(onSelectBlock).toHaveBeenCalledWith('hero');
  });

  test('does not show hover toolbar when isDesignMode is false', () => {
    render(
      <PageCanvas
        content={createPageContent({
          root: {
            uid: 'root-1',
            payload: {
              blocks: [
                {
                  id: 'hero',
                  renderer_version: 'v1',
                  codeRef: 'hero-code',
                  contributionCode: 'official.hero',
                  runtime: 'inline'
                }
              ]
            }
          }
        })}
        isDesignMode={false}
      />
    );

    // The toolbar buttons should not be present
    expect(
      screen.queryByRole('button', { name: '移动或排序区块' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '更多区块操作' })
    ).not.toBeInTheDocument();
  });

  test('renders design mode with hover toolbar actions', () => {
    const designActions = {
      onEditCode: vi.fn(),
      onDelete: vi.fn()
    };

    render(
      <PageCanvas
        content={createPageContent({
          root: {
            uid: 'root-1',
            payload: {
              blocks: [
                {
                  id: 'hero',
                  codeRef: 'hero-code',
                  contributionCode: 'official.hero',
                  runtime: { kind: 'iframe', entry: 'blocks/hero.js' },
                  layout: { order: 10, region: 'main' }
                },
                {
                  id: 'cta',
                  renderer_version: 'v1',
                  codeRef: 'cta-code',
                  contributionCode: 'official.cta',
                  runtime: { kind: 'iframe', entry: 'blocks/cta.js' },
                  layout: { order: 20, region: 'footer' }
                }
              ]
            }
          }
        })}
        isDesignMode
        designActions={designActions}
      />
    );

    // In design mode, blocks are rendered as buttons (container + toolbar buttons)
    const renderSlots = screen.getByTestId('page-canvas-render-slots');
    expect(renderSlots).toBeInTheDocument();
    // Each block container is a role="button" (2 total for 2 blocks)
    const blockButtons = within(renderSlots).getAllByRole('button', {
      name: /区块 /
    });
    expect(blockButtons).toHaveLength(2);
    expect(screen.getByTestId('block-slot-hero').parentElement).toHaveClass(
      'react-grid-item'
    );
    expect(screen.getByTestId('block-slot-hero')).toHaveStyle({
      height: 'auto'
    });
    expect(
      screen.getAllByTestId('frontstage-grid-resize-handle-e')
    ).toHaveLength(2);
    expect(
      screen.getAllByTestId('frontstage-grid-resize-handle-w')
    ).toHaveLength(2);
    expect(
      renderSlots.querySelector('.react-resizable-handle-se')
    ).not.toBeInTheDocument();
  });
});
