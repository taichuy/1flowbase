import { Progress } from 'antd';
import { useTranslation } from 'react-i18next';
import type { RunArchiveImportState } from './application-logs-controls';

export function ApplicationLogsArchiveImportStatus({
  state: archiveImportState
}: {
  state: RunArchiveImportState | null;
}) {
  const { t } = useTranslation('applications');
  return archiveImportState ? (
    <div className="application-logs-page__archive-import-status" role="status">
      <span className="application-logs-page__archive-import-status-text">
        {archiveImportState.phase === 'uploading'
          ? t('auto.import_run_archive_uploading', {
              value1: archiveImportState.fileName
            })
          : t('auto.import_run_archive_processing', {
              value1: archiveImportState.fileName,
              value2: archiveImportState.jobStatus ?? 'queued'
            })}
      </span>
      <Progress
        className="application-logs-page__archive-import-progress"
        percent={archiveImportState.percent}
        size="small"
      />
    </div>
  ) : null;
}
