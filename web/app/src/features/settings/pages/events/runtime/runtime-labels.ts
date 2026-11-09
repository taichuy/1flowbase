import type { EventSourceScope } from '@1flowbase/api-client';
import { useTranslation } from 'react-i18next';

export const scopeKey = (scope: EventSourceScope) =>
  scope.kind === 'workspace' ? `workspace:${scope.workspace_id}` : scope.kind;
export const activityKey = (row: {
  event_id: string;
  phase: string;
  subscription_id: string | null;
  consumer_id: string | null;
  admission: string | null;
  observed_at_unix_ms: number;
}) =>
  JSON.stringify([
    row.event_id,
    row.phase,
    row.subscription_id,
    row.consumer_id,
    row.admission,
    row.observed_at_unix_ms
  ]);
export const timeLabel = (value: number) => new Date(value).toLocaleString();

export function useRuntimeLabels() {
  const { t } = useTranslation('settingsEvents');
  const phases = {
    subscribed: t('subscribed'),
    published: t('published'),
    consumed: t('consumed')
  };
  const admissions = {
    accepted: t('accepted'),
    reentrant: t('reentrant'),
    no_consumer: t('no_consumer'),
    closed: t('closed'),
    resource_exhausted: t('resource_exhausted'),
    unconfigured: t('unconfigured'),
    invalid_body: t('invalid_body')
  };
  const scopeLabel = (scope: EventSourceScope, full = false) =>
    scope.kind === 'workspace'
      ? full
        ? `${t('workspace_scope')} · ${scope.workspace_id}`
        : t('workspace_scope')
      : scope.kind === 'system'
        ? t('system')
        : t('unbound');
  return { t, phases, admissions, scopeLabel };
}
