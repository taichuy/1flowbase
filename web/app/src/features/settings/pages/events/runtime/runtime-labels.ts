import type { EventSourceScope, EventWorkflowStatus } from '@1flowbase/api-client';
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
        ? `${t('workspace_id')} · ${scope.workspace_id}`
        : t('workspace_id')
      : scope.kind === 'system'
        ? t('core')
        : t('unbound');
  return { t, phases, admissions, scopeLabel };
}

export function useWorkflowStatusLabels() {
  const { t } = useTranslation('settingsEvents');
  const activation = (value: EventWorkflowStatus['activation']) =>
    ({
      active: t('workflow_active'),
      unavailable: t('workflow_unavailable'),
      draining: t('workflow_draining'),
      stopping: t('workflow_stopping'),
      stopped: t('workflow_stopped')
    })[value];
  const reason = (value: string | null) => {
    switch (value) {
      case null:
        return t('workflow_reason_absent');
      case 'event_unavailable':
        return t('reason_event_unavailable');
      case 'event_phase_unavailable':
        return t('reason_phase_unavailable');
      case 'invalid_event_input_mapping':
      case 'input_mapping':
        return t('reason_input_mapping');
      case 'application_publication':
      case 'application_unpublished':
        return t('reason_application_unpublished');
      case 'application':
        return t('reason_application_unavailable');
      case 'compiled_plan':
        return t('reason_compiled_plan');
      case 'workflow_trigger_type':
        return t('reason_trigger_type');
      case 'application_disabled':
        return t('reason_application_disabled');
      case 'trigger_disabled':
        return t('reason_trigger_disabled');
      case 'permission_denied':
        return t('reason_permission_denied');
      case 'workspace_access_denied':
      case 'workspace_changed':
      case 'workspace_id':
        return t('reason_workspace_unavailable');
      case 'subscription_author_unavailable':
        return t('reason_author_unavailable');
      case 'subscription_author_disabled':
        return t('reason_author_disabled');
      case 'event_runtime_unconfigured':
        return t('reason_runtime_unconfigured');
      case 'event_runtime_resource_exhausted':
        return t('reason_resource_exhausted');
      case 'event_subscription_closed':
        return t('reason_subscription_closed');
      case 'event_consumer_unavailable':
        return t('reason_consumer_unavailable');
      case 'event_runtime_unavailable':
        return t('reason_runtime_unavailable');
      case 'activation_failed':
        return t('reason_activation_failed');
      case 'receiver_failed':
      case 'receiver_task_failed':
        return t('reason_receiver_failed');
      case 'refresh_unavailable':
      case 'configuration_refresh_failed':
        return t('reason_refresh_unavailable');
      case 'subscription_snapshot_changed':
        return t('reason_snapshot_changed');
      case 'owner_stopped':
        return t('reason_owner_stopped');
      case 'not_authenticated':
        return t('reason_not_authenticated');
      case 'plugin_unavailable':
        return t('reason_plugin_unavailable');
      case 'invalid_component_export':
        return t('reason_component_export');
      case 'invalid_state_transition':
        return t('reason_state_transition');
      default:
        return t('workflow_reason_unavailable');
    }
  };
  return { activation, reason };
}
