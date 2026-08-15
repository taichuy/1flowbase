import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, test } from 'vitest';

import { useAssistantPageReferenceSelection } from '../../components/embedded-assistant/useAssistantPageReferenceSelection';
import { pageReferenceByteLength } from '../../components/debug-console/conversation/PageReferenceTag';

function SelectionHarness({
  maxBytes = 65_536,
  maxCount = 5,
  maxTotalBytes = 65_536,
  pageKey = '/logs'
}: {
  maxBytes?: number;
  maxCount?: number;
  maxTotalBytes?: number;
  pageKey?: string;
}) {
  const selection = useAssistantPageReferenceSelection({
    active: true,
    duplicateMessage: '该元素已引用',
    maxBytes,
    maxCount,
    maxTotalBytes,
    pageKey,
    selectionHint: '选择页面区域',
    tooManyMessage: (max) => `count > ${max}`,
    tooLargeMessage: (actual, max) => `${actual} > ${max}`,
    totalTooLargeMessage: (actual, max) => `total ${actual} > ${max}`,
    unsupportedIsolatedFrameMessage: '隔离区块暂不支持内部元素引用'
  });
  return (
    <div>
      <div data-testid="outer-div">
        <span data-testid="inner-span">内容</span>
        <button data-testid="second-element">重试</button>
      </div>
      <div data-assistant-page-reference-chrome="true">
        <button onClick={selection.startSelection}>开始选择</button>
      </div>
      <div data-testid="selected-html">
        {selection.references.map((reference, index) => (
          <div key={reference.outer_html}>
            <span>{reference.outer_html}</span>
            <button onClick={() => selection.removeReference(index)}>
              删除 {index + 1}
            </button>
          </div>
        ))}
      </div>
      <span data-testid="selection-error">{selection.error ?? ''}</span>
    </div>
  );
}

describe('assistant page reference selection', () => {
  test('reports UTF-8 bytes for the reference summary', () => {
    expect(pageReferenceByteLength('<div>中文</div>')).toBe(17);
  });

  test('AC-001 selects the actual rendered element and Escape cancels without selecting assistant chrome', () => {
    render(<SelectionHarness />);
    fireEvent.click(screen.getByRole('button', { name: '开始选择' }));
    expect(
      document.querySelector('[data-testid="assistant-page-reference-outline"]')
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '开始选择' }));
    expect(screen.getByTestId('selected-html')).toBeEmptyDOMElement();
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(
      document.querySelector('[data-testid="assistant-page-reference-outline"]')
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '开始选择' }));
    fireEvent.click(screen.getByTestId('inner-span'));
    expect(screen.getByTestId('selected-html')).toHaveTextContent(
      '<span data-testid="inner-span">内容</span>'
    );
  });

  test('AC-002 rejects oversized HTML without truncation and clears a draft on page change', () => {
    const { rerender } = render(<SelectionHarness pageKey="/logs" />);
    fireEvent.click(screen.getByRole('button', { name: '开始选择' }));
    fireEvent.click(screen.getByTestId('inner-span'));
    expect(screen.getByTestId('selected-html')).not.toBeEmptyDOMElement();

    rerender(<SelectionHarness pageKey="/monitoring" />);
    expect(screen.getByTestId('selected-html')).toBeEmptyDOMElement();

    rerender(<SelectionHarness maxBytes={10} pageKey="/monitoring" />);
    fireEvent.click(screen.getByRole('button', { name: '开始选择' }));
    fireEvent.click(screen.getByTestId('inner-span'));
    expect(screen.getByTestId('selected-html')).toBeEmptyDOMElement();
    expect(screen.getByTestId('selection-error')).toHaveTextContent(/> 10/);
  });

  test('AC-003 selects the deepest rendered element inside an open ShadowRoot', () => {
    render(<SelectionHarness />);
    const host = document.createElement('section');
    const shadowRoot = host.attachShadow({ mode: 'open' });
    const heading = document.createElement('h1');
    heading.textContent = '区块标题';
    shadowRoot.append(heading);
    document.body.append(host);

    fireEvent.click(screen.getByRole('button', { name: '开始选择' }));
    fireEvent.click(heading);

    expect(screen.getByTestId('selected-html')).toHaveTextContent(
      '<h1>区块标题</h1>'
    );
    host.remove();
  });

  test('AC-004 rejects an isolated iframe instead of referencing its outer element', () => {
    render(<SelectionHarness />);
    const iframe = document.createElement('iframe');
    iframe.setAttribute('sandbox', 'allow-scripts');
    document.body.append(iframe);

    fireEvent.click(screen.getByRole('button', { name: '开始选择' }));
    const iframeOverlay = screen.getByTestId(
      'assistant-page-reference-isolated-frame-overlay'
    );
    fireEvent.click(iframeOverlay);

    expect(screen.getByTestId('selected-html')).toBeEmptyDOMElement();
    expect(screen.getByTestId('selection-error')).toHaveTextContent(
      '隔离区块暂不支持内部元素引用'
    );
    expect(
      document.querySelector('[data-testid="assistant-page-reference-outline"]')
    ).not.toBeInTheDocument();
    iframe.remove();
  });

  test('AC-007 appends references in selection order, rejects duplicates, and removes one draft', () => {
    render(<SelectionHarness />);
    fireEvent.click(screen.getByRole('button', { name: '开始选择' }));
    fireEvent.click(screen.getByTestId('inner-span'));
    fireEvent.click(screen.getByRole('button', { name: '开始选择' }));
    fireEvent.click(screen.getByTestId('second-element'));

    expect(screen.getByTestId('selected-html')).toHaveTextContent(
      '<span data-testid="inner-span">内容</span>'
    );
    expect(screen.getByTestId('selected-html')).toHaveTextContent(
      '<button data-testid="second-element">重试</button>'
    );

    fireEvent.click(screen.getByRole('button', { name: '开始选择' }));
    fireEvent.click(screen.getByTestId('second-element'));
    expect(screen.getByTestId('selection-error')).toHaveTextContent(
      '该元素已引用'
    );

    fireEvent.click(screen.getByRole('button', { name: '删除 1' }));
    expect(screen.getByTestId('selected-html')).not.toHaveTextContent(
      '<span data-testid="inner-span">内容</span>'
    );
    expect(screen.getByTestId('selected-html')).toHaveTextContent(
      '<button data-testid="second-element">重试</button>'
    );
  });

  test('AC-008 enforces the settings count and total byte limits', () => {
    const { rerender } = render(<SelectionHarness maxCount={1} />);
    fireEvent.click(screen.getByRole('button', { name: '开始选择' }));
    fireEvent.click(screen.getByTestId('inner-span'));
    fireEvent.click(screen.getByRole('button', { name: '开始选择' }));
    expect(screen.getByTestId('selection-error')).toHaveTextContent(
      'count > 1'
    );

    rerender(
      <SelectionHarness
        maxCount={5}
        maxTotalBytes={pageReferenceByteLength(
          '<span data-testid="inner-span">内容</span>'
        )}
        pageKey="/monitoring"
      />
    );
    fireEvent.click(screen.getByRole('button', { name: '开始选择' }));
    fireEvent.click(screen.getByTestId('inner-span'));
    fireEvent.click(screen.getByRole('button', { name: '开始选择' }));
    fireEvent.click(screen.getByTestId('second-element'));
    expect(screen.getByTestId('selection-error')).toHaveTextContent(/total/);
    expect(screen.getByTestId('selected-html')).not.toHaveTextContent(
      '<button data-testid="second-element">重试</button>'
    );
  });
});
