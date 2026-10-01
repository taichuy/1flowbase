import { createPortal } from 'react-dom';
import { useEffect, useLayoutEffect, useMemo, useState } from 'react';
import type { BlockContextSizing } from '@1flowbase/page-protocol';
import { useQuery } from '@tanstack/react-query';
import { Alert, App, Button, ConfigProvider, Spin } from 'antd';
import type { ConsoleApplicationType } from '@1flowbase/api-client';
import { useTranslation } from 'react-i18next';

import {
  applicationsQueryKey,
  fetchApplications
} from '../../../api/applications';
import { useNativeBlockSurface } from '../../../../frontstage/lib/native-modules/native-block-surface-context';
import { shadowViewerStyles } from '../ApplicationRunLogViewer';
import { ApplicationLogsWorkspace } from './ApplicationLogsWorkspace';
import tableStyles from '../../../../../shared/ui/data-table/data-table.css?inline';
import selectStyles from '../../../../../shared/ui/autosize-select/autosize-select.css?inline';
import nativeStyles from './application-logs-native.css?inline';

const styles = [
  shadowViewerStyles,
  tableStyles,
  selectStyles,
  nativeStyles
].join('\n');

/** Public native React module; feature owners retain the data and interaction logic. */
export function ApplicationLogs({
  applicationId,
  applicationType = 'agent_flow',
  sizing
}: {
  applicationId: string;
  applicationType?: ConsoleApplicationType;
  sizing?: BlockContextSizing;
}) {
  return (
    <NativeLogsSurface
      applicationId={applicationId}
      applicationType={applicationType}
      sizing={sizing}
    />
  );
}

export function AllAgentFlowLogs({
  sizing
}: { sizing?: BlockContextSizing } = {}) {
  const { t } = useTranslation('applications');
  const applicationsQuery = useQuery({
    queryKey: applicationsQueryKey,
    queryFn: fetchApplications
  });
  const applications = useMemo(
    () =>
      (applicationsQuery.data ?? []).filter(
        (app) => app.application_type === 'agent_flow'
      ),
    [applicationsQuery.data]
  );
  const applicationIds = useMemo(
    () => applications.map((app) => app.id),
    [applications]
  );

  if (applicationsQuery.isPending) return <Spin />;
  if (applicationsQuery.isError) {
    return (
      <Alert
        type="error"
        showIcon
        message={t('auto.application_list_load_failed')}
        action={
          <Button onClick={() => void applicationsQuery.refetch()}>
            {t('auto.refresh_logs')}
          </Button>
        }
      />
    );
  }
  return (
    <NativeLogsSurface
      applicationId=""
      applicationIds={applicationIds}
      applications={applications}
      sizing={sizing}
    />
  );
}

function NativeLogsSurface({
  sizing,
  ...props
}: Parameters<typeof ApplicationLogsWorkspace>[0] & {
  sizing?: BlockContextSizing;
}) {
  const surface = useNativeBlockSurface();
  const [preferredHeight, setPreferredHeight] = useState(() =>
    Math.max(420, window.innerHeight - 212)
  );
  const reportIntrinsicSize = sizing?.reportIntrinsicSize;
  useEffect(() => {
    const updateHeight = () =>
      setPreferredHeight(Math.max(420, window.innerHeight - 212));
    window.addEventListener('resize', updateHeight);
    return () => window.removeEventListener('resize', updateHeight);
  }, []);
  useLayoutEffect(() => {
    reportIntrinsicSize?.({ height: preferredHeight });
  }, [preferredHeight, reportIntrinsicSize]);
  // The host owns frame spacing; content consumes its allocated size after reporting intent.
  const allocatedHeight = sizing?.available.height;
  const { t } = useTranslation('applications');
  if (!surface) return <ApplicationLogsWorkspace {...props} />;

  return (
    <ConfigProvider
      prefixCls="ant"
      getPopupContainer={() => surface.overlayHost.container}
    >
      <App>
        <style data-application-logs-styles>{styles}</style>
        {createPortal(
          <style data-application-logs-overlay-styles>{styles}</style>,
          surface.overlayHost.container
        )}
        <div
          style={{
            height:
              allocatedHeight && allocatedHeight > 0
                ? allocatedHeight
                : preferredHeight
          }}
          className="application-logs-native"
          role="region"
          aria-label={
            props.applicationIds
              ? t('logs.all_agent_flow_applications')
              : undefined
          }
        >
          <ApplicationLogsWorkspace
            {...props}
            overlayContainer={surface.overlayHost.container}
          />
        </div>
      </App>
    </ConfigProvider>
  );
}
