import { fireEvent, screen, within } from '@testing-library/react';

export async function openTrajectoryExecution(nodeDetail: HTMLElement) {
  fireEvent.click(
    await within(nodeDetail).findByRole('button', { name: '供应商轨迹' })
  );
  const dialogs = await screen.findAllByRole('dialog', { name: '供应商轨迹' });
  const dialog = dialogs[dialogs.length - 1]!;
  fireEvent.click(within(dialog).getByRole('tab', { name: '执行关联' }));
  return dialog;
}

export async function openPayloadSection(
  nodeDetail: HTMLElement,
  section: '输入' | '数据处理' | '输出'
) {
  fireEvent.click(
    await within(nodeDetail).findByText(section, { exact: true })
  );
}
