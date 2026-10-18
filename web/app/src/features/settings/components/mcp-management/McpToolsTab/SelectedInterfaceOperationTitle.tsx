import type { ConsoleMcpInterfaceCapability } from '@1flowbase/api-client';
import { Typography } from 'antd';

import { interfaceOptionLabel } from './tool-editor-model';

export function SelectedInterfaceOperationTitle({
  selectedInterface
}: {
  selectedInterface: ConsoleMcpInterfaceCapability | undefined;
}) {
  if (!selectedInterface) return null;
  return (
    <Typography.Text>{interfaceOptionLabel(selectedInterface)}</Typography.Text>
  );
}
