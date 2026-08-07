import { Button, Input, Space, Typography } from 'antd';
import { useState } from 'react';

import type {
  ConsoleCallbackTask,
  ConsoleFlowRunDetail,
  ConsoleRunCheckpoint
} from '@1flowbase/api-client';
import { i18nText } from '../../../../shared/i18n/text';

interface ApplicationRunResumeCardDetail {
  flow_run: Pick<ConsoleFlowRunDetail, 'status'>;
  checkpoints: Array<
    Pick<ConsoleRunCheckpoint, 'id' | 'locator_payload' | 'external_ref_payload'>
  >;
  callback_tasks: Array<
    Pick<ConsoleCallbackTask, 'id' | 'status' | 'callback_kind'>
  >;
}

export function ApplicationRunResumeCard({
  detail,
  onResume,
  onCompleteCallback
}: {
  detail: ApplicationRunResumeCardDetail;
  onResume: (
    checkpointId: string,
    inputPayload: Record<string, unknown>
  ) => Promise<unknown>;
  onCompleteCallback: (
    callbackTaskId: string,
    responsePayload: Record<string, unknown>
  ) => Promise<unknown>;
}) {
  const [humanInput, setHumanInput] = useState('');
  const [callbackJson, setCallbackJson] = useState('{\n  "result": {}\n}');
  const latestCheckpoint = detail.checkpoints[detail.checkpoints.length - 1] ?? null;
  const pendingCallback =
    detail.callback_tasks.find((task) => task.status === 'pending') ?? null;

  if (detail.flow_run.status === 'waiting_human' && latestCheckpoint) {
    const waitingNodeId =
      (latestCheckpoint.locator_payload?.node_id as string | undefined) ?? 'node-human';

    return (
      <div>
        <Typography.Title level={5}>{i18nText("applications", "auto.continue_execution")}</Typography.Title>
        <Space orientation="vertical" style={{ width: '100%' }}>
          <Typography.Text>
            {(latestCheckpoint.external_ref_payload?.prompt as string | undefined) ??
              i18nText("applications", "auto.manual_input_required")}
          </Typography.Text>
          <Input.TextArea
            aria-label={i18nText("applications", "auto.manual_input")}
            rows={4}
            value={humanInput}
            onChange={(event) => setHumanInput(event.target.value)}
          />
          <Button
            type="primary"
            onClick={() =>
              void onResume(latestCheckpoint.id, {
                [waitingNodeId]: { input: humanInput }
              })
            }
          >
            {i18nText("applications", "auto.submit_and_continue")}</Button>
        </Space>
      </div>
    );
  }

  if (detail.flow_run.status === 'waiting_callback' && pendingCallback) {
    return (
      <div>
        <Typography.Title level={5}>{i18nText("applications", "auto.callback_backfill")}</Typography.Title>
        <Space orientation="vertical" style={{ width: '100%' }}>
          <Typography.Text>{pendingCallback.callback_kind}</Typography.Text>
          <Input.TextArea
            aria-label={i18nText("applications", "auto.callback_response")}
            rows={6}
            value={callbackJson}
            onChange={(event) => setCallbackJson(event.target.value)}
          />
          <Button
            type="primary"
            onClick={() =>
              void onCompleteCallback(
                pendingCallback.id,
                JSON.parse(callbackJson) as Record<string, unknown>
              )
            }
          >
            {i18nText("applications", "auto.backfill_and_continue")}</Button>
        </Space>
      </div>
    );
  }

  return null;
}
