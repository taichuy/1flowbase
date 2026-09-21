import { useState } from 'react';
import { Button, Modal, Tooltip, Segmented } from 'antd';
import ApartmentOutlined from '@ant-design/icons/es/icons/ApartmentOutlined';
import type { ConversationLogTraceLoader } from '../conversation-log-trace-model';
import { i18nText } from '../../../../../shared/i18n/text';
import { useWindowWorkspaceOverlayZIndex } from '../../../../../shared/ui/window-workspace/WindowWorkspaceWindow';
import { NativeTrajectoryWorkspace } from './NativeTrajectoryWorkspace';
import { ClientTrajectoryWorkspace } from './client/ClientTrajectoryWorkspace';
import './provider-trajectory.css';
/** A trajectory augments the existing workflow tree; it never owns that tree. */
export function ProviderTrajectory({
  runId,
  nodeRunId,
  loader,
  compatibility_mode
}: {
  runId: string;
  nodeRunId?: string;
  compatibility_mode?: string;
  loader: ConversationLogTraceLoader;
}) {
  const zIndex = useWindowWorkspaceOverlayZIndex();
  const [source, setSource] = useState<'client' | 'native'>('client');
  const [open, setOpen] = useState(false);
  const title = nodeRunId
    ? i18nText('agentFlow', 'trajectory.title')
    : i18nText('agentFlow', 'trajectory.run_title');
  return (
    <>
      <Tooltip title={title}>
        <Button
          size="small"
          type="text"
          icon={<ApartmentOutlined />}
          aria-label={title}
          title={title}
          onClick={(event) => {
            event.stopPropagation();
            setOpen(true);
          }}
        />
      </Tooltip>
      <Modal
        zIndex={zIndex}
        open={open}
        onCancel={() => setOpen(false)}
        footer={null}
        width="min(1440px, calc(100vw - 32px))"
        title={compatibility_mode ? `${title} · ${compatibility_mode}` : title}
        destroyOnHidden
        styles={{ body: { padding: 0, minHeight: 0 } }}
      >
        {open ? (
          <>
            <Segmented
              className="client-trajectory__source"
              value={source}
              onChange={(value) => setSource(value as 'client' | 'native')}
              options={[
                {
                  value: 'client',
                  label: i18nText('agentFlow', 'client_trajectory.client')
                },
                {
                  value: 'native',
                  label: i18nText('agentFlow', 'client_trajectory.native')
                }
              ]}
            />
            {source === 'client' ? (
              <ClientTrajectoryWorkspace
                runId={runId}
                nodeRunId={nodeRunId}
                loader={loader}
              />
            ) : (
              <NativeTrajectoryWorkspace
                runId={runId}
                nodeRunId={nodeRunId}
                loader={loader}
              />
            )}
          </>
        ) : null}
      </Modal>
    </>
  );
}
