import DownOutlined from '@ant-design/icons/es/icons/DownOutlined';
import FileTextOutlined from '@ant-design/icons/es/icons/FileTextOutlined';
import RightOutlined from '@ant-design/icons/es/icons/RightOutlined';
import { Tag, Typography } from 'antd';
import { useState } from 'react';

import type { AgentFlowAnswerSnapshot } from '../../../api/runtime';
import {
  RuntimeDebugPayloadBlock,
  type RuntimeDebugArtifactBatchLoader
} from '../../detail/last-run/runtime-debug-payload';

function snapshotPayload(snapshot: AgentFlowAnswerSnapshot) {
  return Object.keys(snapshot.outputPayload).length > 0
    ? snapshot.outputPayload
    : { answer: snapshot.text };
}

function snapshotStatus(snapshot: AgentFlowAnswerSnapshot) {
  return snapshot.complete ? '已完成' : '等待中';
}

export function AnswerSnapshotTrace({
  snapshot,
  onLoadArtifact,
  onLoadArtifacts
}: {
  snapshot: AgentFlowAnswerSnapshot;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
}) {
  const [expanded, setExpanded] = useState(false);

  return (
    <section
      aria-label="answer快照"
      className="agent-flow-editor__debug-answer-snapshot"
    >
      <button
        aria-expanded={expanded}
        className="agent-flow-editor__debug-answer-snapshot-trigger"
        onClick={() => setExpanded((current) => !current)}
        type="button"
      >
        <span className="agent-flow-editor__debug-answer-snapshot-title">
          <FileTextOutlined className="agent-flow-editor__debug-answer-snapshot-icon" />
          <Typography.Text strong>answer快照</Typography.Text>
          <Tag color={snapshot.complete ? 'success' : 'warning'}>
            {snapshotStatus(snapshot)}
          </Tag>
        </span>
        {expanded ? (
          <DownOutlined className="agent-flow-editor__debug-workflow-collapse" />
        ) : (
          <RightOutlined className="agent-flow-editor__debug-workflow-collapse" />
        )}
      </button>
      {expanded ? (
        <div className="agent-flow-editor__debug-answer-snapshot-body">
          <RuntimeDebugPayloadBlock
            height="11rem"
            payload={snapshotPayload(snapshot)}
            title="answer快照"
            onLoadArtifact={onLoadArtifact}
            onLoadArtifacts={onLoadArtifacts}
          />
        </div>
      ) : null}
    </section>
  );
}
