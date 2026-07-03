import { render, screen } from '@testing-library/react';
import { type CSSProperties, type ReactNode } from 'react';
import { describe, expect, test, vi } from 'vitest';

import { FixedHeightModal } from '../FixedHeightModal';

const antdMocks = vi.hoisted(() => ({
  Modal: vi.fn()
}));

vi.mock('antd', async () => {
  const actual = await vi.importActual<typeof import('antd')>('antd');

  antdMocks.Modal.mockImplementation(
    ({
      children,
      className,
      footer,
      open,
      style,
      title
    }: {
      children?: ReactNode;
      className?: string;
      footer?: ReactNode;
      open?: boolean;
      style?: CSSProperties;
      title?: ReactNode;
    }) =>
      open ? (
        <div className={className} data-testid="mock-modal" style={style}>
          <div>{title}</div>
          {children}
          <div>{footer}</div>
        </div>
      ) : null
  );

  return {
    ...actual,
    Modal: antdMocks.Modal
  };
});

describe('FixedHeightModal', () => {
  test('centralizes fixed modal shell sizing and scroll body structure', () => {
    const footer = <button type="button">保存</button>;

    render(
      <FixedHeightModal
        className="domain-modal"
        footer={footer}
        height="640px"
        open
        title="工具配置"
        width={840}
        bodyHeader={<div>步骤导航</div>}
        onCancel={vi.fn()}
      >
        <div>表单内容</div>
      </FixedHeightModal>
    );

    expect(antdMocks.Modal.mock.calls.at(0)?.[0]).toMatchObject({
      centered: true,
      open: true,
      title: '工具配置',
      width: 840
    });
    expect(antdMocks.Modal.mock.calls.at(0)?.[0].className).toBe(
      'fixed-height-modal domain-modal'
    );
    expect(antdMocks.Modal.mock.calls.at(0)?.[0].style).toMatchObject({
      '--fixed-height-modal-content-height': '640px'
    });

    const scrollBody = screen.getByTestId('fixed-height-modal-scroll-body');

    expect(scrollBody).toHaveClass('fixed-height-modal__scroll-body');
    expect(screen.getByTestId('fixed-height-modal-body-header')).toHaveClass(
      'fixed-height-modal__body-header'
    );
    expect(screen.getByText('步骤导航')).toBeInTheDocument();
    expect(screen.getByText('表单内容')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '保存' })).toBeInTheDocument();
  });
});
