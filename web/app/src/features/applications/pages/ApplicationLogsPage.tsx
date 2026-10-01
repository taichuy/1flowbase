import type { ConsoleApplicationType } from '@1flowbase/api-client';
import { ApplicationLogsWorkspace } from '../components/logs/workspace/ApplicationLogsWorkspace';

export function ApplicationLogsPage(props: {
  applicationId: string;
  applicationType?: ConsoleApplicationType;
}) {
  return <ApplicationLogsWorkspace {...props} />;
}
