import { useState } from 'react';
import { Button, Modal, Tooltip, Segmented } from 'antd';
import ApartmentOutlined from '@ant-design/icons/es/icons/ApartmentOutlined';
import type { ProviderTrajectoryStep } from '@1flowbase/api-client';
import type { ConversationLogTraceLoader } from '../conversation-log-trace-model';
import { i18nText } from '../../../../../shared/i18n/text';
import { useWindowWorkspaceOverlayZIndex } from '../../../../../shared/ui/window-workspace/WindowWorkspaceWindow';
import { NativeTrajectoryWorkspace } from './NativeTrajectoryWorkspace';
import { ClientTrajectoryWorkspace } from './client/ClientTrajectoryWorkspace';
import './provider-trajectory.css';
type View = {
  source: 'client' | 'native';
  runId: string;
  nodeRunId?: string;
  request_id?: string;
  focus_step_id?: string;
  focus_event_id?: string;
};

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
  const initialSource = nodeRunId ? 'native' : 'client';
  const [views, setViews] = useState<View[]>([
    { source: initialSource, runId, nodeRunId }
  ]);
  const [history, setHistory] = useState([0]);
  const active = history[history.length - 1];
  const source = views[active].source;
  function navigate(view: View) {
    setViews((current) => [...current, view]);
    setHistory((current) => [...current, views.length]);
  }
  function switchSource(next: 'client' | 'native') {
    if (next === source) return;
    const existing = views.findIndex(
      (view) => view.source === next && !view.request_id
    );
    if (existing >= 0) setHistory((current) => [...current, existing]);
    else navigate({ source: next, runId, nodeRunId });
  }
  function openClient(
    link: NonNullable<ProviderTrajectoryStep['links']>[number]
  ) {
    navigate({
      source: 'client',
      runId: link.flow_run_id,
      request_id: link.request_id,
      focus_step_id: link.request_id
    });
  }
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
        onCancel={() => {
          setOpen(false);
          setViews([{ source: initialSource, runId, nodeRunId }]);
          setHistory([0]);
        }}
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
              onChange={(value) => switchSource(value as 'client' | 'native')}
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
            {history.length > 1 ? (
              <Button
                type="link"
                onClick={() => setHistory((current) => current.slice(0, -1))}
              >
                {i18nText('agentFlow', 'trajectory.return_view')}
              </Button>
            ) : null}
            {views.map((view, index) => (
              <div key={index} hidden={index !== active}>
                {view.source === 'client' ? (
                  <ClientTrajectoryWorkspace
                    runId={view.runId}
                    nodeRunId={view.nodeRunId}
                    options={
                      view.request_id
                        ? {
                            request_id: view.request_id,
                            focus_step_id: view.focus_step_id
                          }
                        : undefined
                    }
                    loader={loader}
                    onInternal={(step, scope) =>
                      navigate({
                        source: 'native',
                        runId: step.flow_run_id,
                        nodeRunId: scope,
                        request_id: step.request_id
                      })
                    }
                  />
                ) : (
                  <NativeTrajectoryWorkspace
                    runId={view.runId}
                    nodeRunId={view.nodeRunId}
                    options={
                      view.request_id
                        ? {
                            request_id: view.request_id,
                            focus_event_id: view.focus_event_id
                          }
                        : undefined
                    }
                    loader={loader}
                    onClient={openClient}
                  />
                )}
              </div>
            ))}
          </>
        ) : null}
      </Modal>
    </>
  );
}
