import { lazy, Suspense } from 'react';
import { Alert, Button, Spin } from 'antd';
import { i18nText } from '../../../../../../shared/i18n/text';
import { unfinishedProjection, type ActivityPages } from './activity-query';

const ProjectionNotice = lazy(() =>
  import('../../ConversationLogPanel').then((module) => ({
    default: module.TraceProjectionStatusNotice
  }))
);
export function PageStatus({
  pages
}: {
  pages: {
    isError: boolean;
    isFetching: boolean;
    refetch: () => Promise<unknown>;
  } & ActivityPages;
}) {
  const projection = unfinishedProjection(pages);
  if (projection)
    return (
      <>
        <Suspense fallback={<Spin size="small" />}>
          <ProjectionNotice status={projection} />
        </Suspense>
        {projection.retriable ? (
          <Button onClick={() => void pages.refetch()}>
            {i18nText('agentFlow', 'auto.retry')}
          </Button>
        ) : null}
      </>
    );
  return pages.isError ? (
    <Alert
      type="error"
      title={i18nText('agentFlow', 'auto.loading_failed')}
      action={
        <Button onClick={() => void pages.refetch()}>
          {i18nText('agentFlow', 'auto.retry')}
        </Button>
      }
    />
  ) : pages.isFetching ? (
    <Spin size="small" />
  ) : null;
}
