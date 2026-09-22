import { useState } from 'react';
import { createPortal } from 'react-dom';
import CloseOutlined from '@ant-design/icons/es/icons/CloseOutlined';
import { Button, Tooltip, Segmented } from 'antd';
import ApartmentOutlined from '@ant-design/icons/es/icons/ApartmentOutlined';
import type { ProviderTrajectoryStep } from '@1flowbase/api-client';
import type { ConversationLogTraceLoader } from '../conversation-log-trace-model';
import { i18nText } from '../../../../../shared/i18n/text';
import {
  WindowWorkspaceWindow,
  useWindowWorkspaceOverlayZIndex
} from '../../../../../shared/ui/window-workspace/WindowWorkspaceWindow';
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
  compatibility_mode,
  buttonType = 'text'
}: {
  runId: string;
  nodeRunId?: string;
  compatibility_mode?: string;
  buttonType?: 'text' | 'default';
  loader: ConversationLogTraceLoader;
}) {
  const zIndex = useWindowWorkspaceOverlayZIndex();
  const initialSource = nodeRunId ? 'native' : 'client';
  const [views, setViews] = useState<View[]>([
    { source: initialSource, runId, nodeRunId }
  ]);
  const [active, setActive] = useState(0);
  const source = views[active].source;
  function navigate(view: View) {
    setViews((current) => [...current, view]);
    setActive(views.length);
  }
  function switchSource(next: 'client' | 'native') {
    if (next === source) return;
    const existing = views.findIndex(
      (view) => view.source === next && !view.request_id
    );
    if (existing >= 0) setActive(existing);
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
          type={buttonType}
          icon={<ApartmentOutlined />}
          aria-label={title}
          title={title}
          onClick={(event) => {
            event.stopPropagation();
            setOpen(true);
          }}
        />
      </Tooltip>
      {open
        ? createPortal(
            <WindowWorkspaceWindow
              active
              zIndex={zIndex ?? 1052}
              title={title}
              testId="trajectory-window"
              className="trajectory-window"
              bodyClassName="trajectory-window__body"
              dragHandleSelector=".trajectory-window__header"
              initialRect={() => ({
                left: Math.max(8, (window.innerWidth - 1440) / 2),
                top: 40,
                width: Math.min(1440, window.innerWidth - 16),
                height: Math.min(900, window.innerHeight - 64)
              })}
              minWidth={340}
              minHeight={320}
              onActivate={() => {}}
              resizeLabel={(edge) =>
                i18nText('agentFlow', 'trajectory.resize_window', { edge })
              }
            >
              <header className="trajectory-window__header">
                <strong>
                  {compatibility_mode
                    ? `${title} · ${compatibility_mode}`
                    : title}
                </strong>
                <Button
                  type="text"
                  icon={<CloseOutlined />}
                  aria-label={i18nText('agentFlow', 'trajectory.close_window')}
                  onClick={() => {
                    setOpen(false);
                    setViews([{ source: initialSource, runId, nodeRunId }]);
                    setActive(0);
                  }}
                />
              </header>
              {open ? (
                <>
                  <Segmented
                    className="client-trajectory__source"
                    value={source}
                    onChange={(value) =>
                      switchSource(value as 'client' | 'native')
                    }
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
                  {views.map((view, index) => (
                    <div
                      className="trajectory-window__view"
                      key={index}
                      hidden={index !== active}
                    >
                      {view.source === 'client' ? (
                        <ClientTrajectoryWorkspace
                          active={index === active}
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
                          active={index === active}
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
            </WindowWorkspaceWindow>,
            document.body
          )
        : null}
    </>
  );
}
