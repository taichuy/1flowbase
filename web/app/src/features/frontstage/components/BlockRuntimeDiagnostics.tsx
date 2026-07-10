import { Alert, List, Space, Typography } from 'antd';
import type { FC } from 'react';
import type { BlockRuntimeDiagnostic } from '@1flowbase/page-protocol';

export interface BlockRuntimeDiagnosticsProps {
  diagnostics: BlockRuntimeDiagnostic[];
}

export const BlockRuntimeDiagnostics: FC<BlockRuntimeDiagnosticsProps> = ({
  diagnostics
}) => {
  if (diagnostics.length === 0) {
    return null;
  }

  return (
    <Alert
      type="error"
      showIcon
      message="代码诊断"
      description={
        <List
          size="small"
          dataSource={diagnostics}
          renderItem={(diagnostic) => (
            <List.Item>
              <Space direction="vertical" size={0}>
                <Typography.Text>{diagnostic.message}</Typography.Text>
                <Typography.Text type="secondary">
                  {formatDiagnosticLocation(diagnostic)}
                </Typography.Text>
              </Space>
            </List.Item>
          )}
        />
      }
    />
  );
};

function formatDiagnosticLocation(diagnostic: BlockRuntimeDiagnostic): string {
  const location = diagnostic.sourceLocation
    ? ` · ${diagnostic.sourceLocation.line}:${diagnostic.sourceLocation.column}`
    : '';
  return `${diagnostic.phase}${location}`;
}
