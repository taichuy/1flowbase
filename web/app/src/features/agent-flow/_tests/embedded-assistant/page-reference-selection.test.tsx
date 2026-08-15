import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, test } from 'vitest';

import { useAssistantPageReferenceSelection } from '../../components/embedded-assistant/useAssistantPageReferenceSelection';
import { pageReferenceByteLength } from '../../components/debug-console/conversation/PageReferenceTag';

function SelectionHarness({
  maxBytes = 65_536,
  pageKey = '/logs'
}: {
  maxBytes?: number;
  pageKey?: string;
}) {
  const selection = useAssistantPageReferenceSelection({
    active: true,
    maxBytes,
    pageKey,
    selectionHint: '选择页面区域',
    tooLargeMessage: (actual, max) => `${actual} > ${max}`
  });
  return (
    <div>
      <div data-testid="outer-div">
        <span data-testid="inner-span">内容</span>
      </div>
      <div data-assistant-page-reference-chrome="true">
        <button onClick={selection.startSelection}>开始选择</button>
      </div>
      <span data-testid="selected-html">
        {selection.reference?.outer_html ?? ''}
      </span>
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
});
