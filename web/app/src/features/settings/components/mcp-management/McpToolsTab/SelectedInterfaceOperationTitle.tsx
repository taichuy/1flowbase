import type { ConsoleMcpInterfaceCapabilitySummary } from '@1flowbase/api-client';
import { Typography } from 'antd';

import { interfaceOptionLabel } from './tool-editor-model';

export function SelectedInterfaceOperationTitle({
  selectedInterface
}: {
  selectedInterface:
    | Pick<ConsoleMcpInterfaceCapabilitySummary, 'method' | 'path'>
    | undefined;
}) {
  if (!selectedInterface) return null;
  return (
    <Typography.Text>{interfaceOptionLabel(selectedInterface)}</Typography.Text>
  );
}
