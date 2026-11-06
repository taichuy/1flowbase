export type EventTriggerPhase = 'subscribed' | 'published' | 'consumed';
export type EventDefinitionOrigin = 'custom' | 'core' | 'plugin';
export type EventSourceScope =
  | { kind: 'workspace'; workspace_id: string }
  | { kind: 'system' }
  | { kind: 'unbound' };
export interface EventSourceSelection {
  source_id: string;
  scope: EventSourceScope;
}
export interface EventCatalogSource {
  selection: EventSourceSelection;
  point_id: string;
  binding: 'declaration' | 'active';
  coverage: 'live' | 'unavailable';
}
export interface EventSourceDescriptor {
  source_id: string;
  owner_id: string;
  point_id: string;
  contract_id: string;
  contract_version: string;
  name: string;
  description: string;
  payload_schema: Record<string, unknown> | boolean | null;
  origin: EventDefinitionOrigin;
  scope: EventSourceScope;
  binding: 'declaration' | 'active';
  coverage: 'live' | 'unavailable';
  supported_phases: EventTriggerPhase[];
}
export interface EventSourceQueueSnapshot {
  queue_id: string;
  subscriber_id: string;
  consumer_ids: string[];
  buffered?: number | null;
  in_flight?: number | null;
  accepted?: number | null;
  dequeued?: number | null;
  dropped?: number | null;
  observed_at_unix_ms: number;
}
export interface EventRegisteredSourceSnapshot {
  queue_observation_complete: boolean;
  metadata_bytes: number;
  registration_id: string;
  descriptor: EventSourceDescriptor;
  queues: EventSourceQueueSnapshot[];
}
export interface EventSourceContext {
  registration_id: string;
  scope: EventSourceScope;
  source_id: string;
  point_id: string;
  original_event_id?: string | null;
  producer_id?: string | null;
}
export interface EventSourceRuntimeSnapshot {
  instance_id: string;
  configured: boolean;
  observation_max_bytes: number;
  observation_ttl_ms: number;
  scope: EventSourceScope;
  sources: EventRegisteredSourceSnapshot[];
  workflow_queues: EventRuntimeQueueSnapshot[];
  activity: EventRuntimeActivity[];
  observation_bytes: number;
  source_metadata_bytes: number;
  observation_loss: EventObservationLossSnapshot;
}
export interface EventCatalogEntry {
  source?: EventCatalogSource;
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
  | 'reentrant'
  | 'no_consumer'
  | 'closed'
  | 'resource_exhausted'
  | 'unconfigured'
  | 'invalid_body';
export interface EventRuntimeQueueSnapshot {
  source_target?: EventSourceSelection & { workflow_id: string };
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
  source_context?: EventSourceContext;
  original_subscriber_id?: string;
  original_consumer_id?: string;
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
export interface EventObservationLossSnapshot {
  accounting_available: boolean;
  window_started_at_unix_ms: number | null;
  window_duration_ms: number;
  expired: number;
  evicted: number;
  oversize_dropped: number;
}
export interface EventRuntimeSnapshot {
  source_queue_refresh_error: string | null;
  source_views: EventSourceRuntimeSnapshot[];
  source_metadata_bytes: number;
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
  observation_loss: EventObservationLossSnapshot;
}
