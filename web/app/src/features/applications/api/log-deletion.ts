import {
  deleteConsoleApplicationLogs,
  type AgentLogsDeleteScope,
  type AgentLogsDeleteReceipt
} from '@1flowbase/api-client';
import { getApplicationsApiBaseUrl } from './applications';

// Sequential committed batches; stopping never aborts or rolls back the active batch.
export async function deleteApplicationLogsInBatches(
  applicationId: string,
  scope: AgentLogsDeleteScope,
  csrfToken: string,
  onReceipt: (receipt: AgentLogsDeleteReceipt) => void,
  shouldStop: () => boolean
): Promise<'complete' | 'stopped'> {
  let ingested_at_before = scope.ingested_at_before;
  while (!shouldStop()) {
    const receipt = await deleteConsoleApplicationLogs(
      applicationId,
      { ...scope, ingested_at_before },
      csrfToken,
      getApplicationsApiBaseUrl()
    );
    onReceipt(receipt);
    ingested_at_before = receipt.ingested_at_before;
    if (!receipt.has_more) return 'complete';
  }
  return 'stopped';
}
