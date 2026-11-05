export type EventTriggerPhase = 'subscribed' | 'published' | 'consumed';
export type EventDefinitionOrigin = 'custom' | 'core' | 'plugin';
export interface EventCatalogEntry {
  id: string | null;
  enabled: boolean;
  owner_id: string;
  contract_id: string;
  contract_version: string;
  name: string;
  description: string;
  payload_schema: Record<string, unknown> | boolean | null;
  origin: EventDefinitionOrigin;
  supported_phases: EventTriggerPhase[];
}
export interface CreateEventDefinitionInput {
  contract_id: string;
  contract_version: string;
  name: string;
  description: string;
  payload_schema: Record<string, unknown> | boolean;
  enabled: boolean;
}
export type UpdateEventDefinitionInput = Omit<
  CreateEventDefinitionInput,
  'contract_id' | 'contract_version'
>;
export interface EventDefinitionRecord extends CreateEventDefinitionInput {
  id: string;
  workspace_id: string;
  created_by: string;
  created_at: string;
  updated_at: string;
}
export type EventTargetAdmissionStatus =
  | 'accepted'
  | 'no_consumer'
  | 'closed'
  | 'resource_exhausted'
  | 'unconfigured'
  | 'invalid_body';
export interface EventRuntimeQueueSnapshot {
  subscription_id: string;
  workspace_id: string;
  contract_id: string;
  contract_version: string;
  trigger_phase: EventTriggerPhase;
  consumer_ids: string[];
  closed: boolean;
  buffered: number;
  in_flight: number;
  accepted: number;
  consumed: number;
  admission_rejected: number;
}
export interface EventRuntimeActivity {
  event_id: string;
  workspace_id: string;
  contract_id: string;
  contract_version: string;
  phase: EventTriggerPhase;
  source_event_id: string | null;
  subscription_id: string | null;
  consumer_id: string | null;
  admission: EventTargetAdmissionStatus | null;
  observed_at_unix_ms: number;
}
export interface EventWorkflowStatus {
  workspace_id: string;
  application_id: string;
  subscription_id: string;
  activation: 'active' | 'unavailable' | 'draining' | 'stopping' | 'stopped';
  reason: string | null;
  last_signal_id: string | null;
  last_run_id: string | null;
  last_delivery: string | null;
  last_delivery_reason: string | null;
}
export interface EventRuntimeSnapshot {
  instance_id: string;
  configured: boolean;
  workspace_id: string;
  queues: EventRuntimeQueueSnapshot[];
  activity: EventRuntimeActivity[];
  workflow_subscriptions: EventWorkflowStatus[];
  workflow_refresh_error: string | null;
  admission_rejected: number;
  observation_bytes: number;
  observation_max_bytes: number;
  observation_ttl_ms: number;
}
