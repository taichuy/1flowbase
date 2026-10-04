import '../../../../../test/fixtures/rc-util-unique-ids';

import { fireEvent, within } from '@testing-library/react';

export async function openPayloadSection(
  nodeDetail: HTMLElement,
  section: '输入' | '数据处理' | '输出'
) {
  fireEvent.click(
    await within(nodeDetail).findByText(section, { exact: true })
  );
}
