import { Card } from 'antd';

import type { NodeLastRun } from '../../../api/runtime';
import { fetchRuntimeDebugArtifacts } from '../../../api/runtime';
import { i18nText } from '../../../../../shared/i18n/text';
import { NodeRunPayloadSections } from './NodeRunPayloadSections';

export function NodeRunIOCard({ lastRun }: { lastRun: NodeLastRun }) {
  const applicationId = lastRun.flow_run.application_id;

  return (
    <Card title={i18nText('agentFlow', 'auto.node_input_output')}>
      <div className="agent-flow-node-run-json-list">
        <NodeRunPayloadSections
          inputPayload={lastRun.node_run.input_payload}
          debugPayload={lastRun.node_run.debug_payload}
          outputPayload={lastRun.node_run.output_payload}
          errorPayload={lastRun.node_run.error_payload}
          onLoadArtifacts={(artifactRefs) =>
            fetchRuntimeDebugArtifacts(applicationId, artifactRefs)
          }
        />
      </div>
    </Card>
  );
}
