import { fireEvent, render, screen } from '@testing-library/react';
import { type ReactNode, useState } from 'react';
import { describe, expect, test, vi } from 'vitest';

import { JsonPreviewBlock } from '../JsonPreviewBlock';

const antdMocks = vi.hoisted(() => ({
  Modal: vi.fn()
}));

vi.mock('@monaco-editor/react', () => ({
  default: ({ value }: { value: string }) => (
    <pre data-testid="mock-json-editor">{value}</pre>
  )
}));

vi.mock('../../../code-block/monaco-runtime', () => ({
  loadMonacoEditorModule: () => import('@monaco-editor/react')
}));

vi.mock('antd', async () => {
  const actual = await vi.importActual<typeof import('antd')>('antd');

  antdMocks.Modal.mockImplementation(
    ({
      children,
      open,
      title,
      zIndex
    }: {
      children?: ReactNode;
      open?: boolean;
      title?: ReactNode;
      zIndex?: number;
    }) =>
      open ? (
        <div
          aria-label={String(title)}
          data-testid="mock-modal"
          data-z-index={zIndex}
        >
          {children}
        </div>
      ) : null
  );

  return {
    ...actual,
    App: {
      useApp: () => ({
        message: {
          error: vi.fn(),
          success: vi.fn()
        }
      })
    },
    Button: ({
      'aria-label': ariaLabel,
      disabled,
      icon,
      onClick
    }: {
      'aria-label'?: string;
      disabled?: boolean;
      icon?: ReactNode;
      onClick?: () => void;
    }) => (
      <button
        aria-label={ariaLabel}
        disabled={disabled}
        onClick={onClick}
        type="button"
      >
        {icon}
      </button>
    ),
    Modal: antdMocks.Modal,
    Tooltip: ({ children }: { children?: ReactNode }) => <>{children}</>
  };
});

describe('JsonPreviewBlock', () => {
  test('opens the enlarged JSON modal above application log floating windows', async () => {
    render(
      <JsonPreviewBlock
        fullscreenAriaLabel="放大查看工具调用 JSON"
        title="工具调用"
        value={{ ok: true }}
      />
    );

    await screen.findByTestId('mock-json-editor');

    fireEvent.click(
      screen.getByRole('button', { name: '放大查看工具调用 JSON' })
    );

    expect(screen.getByTestId('mock-modal')).toHaveAttribute(
      'data-z-index',
      '1060'
    );
    expect(antdMocks.Modal.mock.calls.at(-1)?.[0]).toMatchObject({
      zIndex: 1060
    });
  });
});

test('embeds actions in the owning section without a second title or collapse', async () => {
  const toggle = vi.fn();
  function EmbeddedPreview() {
    const [target, setTarget] = useState<HTMLDivElement | null>(null);
    return (
      <>
        <div onClick={toggle} onKeyDown={toggle}>
          <h4>Output</h4>
          <div ref={setTarget} data-testid="section-actions" />
        </div>
        <JsonPreviewBlock
          title="Output"
          value={{ ok: true }}
          defaultCollapsed
          headerActionsTarget={target}
          fullscreenAriaLabel="Enlarge output"
        />
      </>
    );
  }
  const { container, unmount } = render(<EmbeddedPreview />);
  await screen.findByTestId('mock-json-editor');
  expect(screen.getAllByText('Output')).toHaveLength(1);
  expect(
    container.querySelector('.json-preview-block__header')
  ).not.toBeInTheDocument();
  const enlarge = screen.getByRole('button', { name: 'Enlarge output' });
  expect(screen.getByTestId('section-actions')).toContainElement(enlarge);
  fireEvent.keyDown(enlarge, { key: 'Enter' });
  fireEvent.click(enlarge);
  expect(toggle).not.toHaveBeenCalled();
  expect(screen.getByTestId('mock-modal')).toBeInTheDocument();
  unmount();
  expect(
    screen.queryByRole('button', { name: 'Enlarge output' })
  ).not.toBeInTheDocument();
});
