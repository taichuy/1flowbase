const BACKEND_SHARDS = [
  {
    key: 'core-libs',
    packages: ['domain', 'access-control', 'observability', 'runtime-profile', 'plugin-framework'],
  },
  {
    key: 'runtime-storage',
    packages: [
      'runtime-core',
      'orchestration-runtime',
      'publish-gateway',
      'storage-durable',
      'storage-ephemeral',
      'storage-object',
      'storage-postgres',
    ],
  },
  {
    key: 'apps',
    packages: ['control-plane', 'api-server', 'plugin-runner'],
  },
];
const BACKEND_APP_TEST_SHARDS = [
  {
    key: 'control-plane',
    packages: ['control-plane'],
  },
  ...Array.from({ length: 4 }, (_, index) => ({
    key: `api-server-${index + 1}-of-4`,
    packages: ['api-server'],
    nextestPartition: `hash:${index + 1}/4`,
  })),
  {
    key: 'plugin-runner',
    packages: ['plugin-runner'],
  },
];
const BACKEND_TEST_SHARDS = [
  ...BACKEND_SHARDS,
  ...BACKEND_APP_TEST_SHARDS,
];
const BACKEND_CI_TEST_SHARDS = [
  BACKEND_SHARDS.find((shard) => shard.key === 'core-libs'),
  BACKEND_SHARDS.find((shard) => shard.key === 'runtime-storage'),
  ...BACKEND_APP_TEST_SHARDS,
];
const BACKEND_CONSISTENCY_TARGETS = [
  {
    label: 'consistency-control-plane-state-transitions',
    packageName: 'control-plane',
    filter: 'state_transition_tests',
  },
  {
    label: 'consistency-control-plane-workspace-session',
    packageName: 'control-plane',
    filter: 'workspace_session',
  },
  {
    label: 'consistency-control-plane-model-definition-service',
    packageName: 'control-plane',
    filter: 'model_definition_service_tests',
  },
  {
    label: 'consistency-control-plane-model-definition-runtime-sync',
    packageName: 'control-plane',
    filter: 'model_definition_runtime_sync_tests',
  },
  {
    label: 'consistency-control-plane-resource-action-kernel',
    packageName: 'control-plane',
    filter: 'resource_action_tests',
  },
  {
    label: 'consistency-runtime-acl',
    packageName: 'runtime-core',
    filter: 'runtime_acl_tests',
  },
  {
    label: 'consistency-runtime-engine',
    packageName: 'runtime-core',
    filter: 'runtime_engine_tests',
  },
  {
    label: 'consistency-storage-migration-smoke',
    packageName: 'storage-postgres',
    filter: 'migration_smoke',
  },
  {
    label: 'consistency-storage-model-definition-repository',
    packageName: 'storage-postgres',
    filter: 'model_definition_repository_tests',
  },
  {
    label: 'consistency-storage-runtime-record-repository',
    packageName: 'storage-postgres',
    filter: 'runtime_record_repository_tests',
  },
  {
    label: 'consistency-storage-orchestration-runtime-repository',
    packageName: 'storage-postgres',
    filter: 'orchestration_runtime_repository_tests',
  },
  {
    label: 'consistency-storage-physical-schema-repository',
    packageName: 'storage-postgres',
    filter: 'physical_schema_repository_tests',
  },
  {
    label: 'consistency-storage-workspace-scope',
    packageName: 'storage-postgres',
    filter: 'workspace_scope_tests',
  },
  {
    label: 'consistency-api-model-definition-routes',
    packageName: 'api-server',
    filter: 'model_definition_routes',
  },
  {
    label: 'consistency-api-runtime-model-routes',
    packageName: 'api-server',
    filter: 'runtime_model_routes',
  },
  {
    label: 'consistency-api-workspace-routes',
    packageName: 'api-server',
    filter: 'workspace_routes',
  },
  {
    label: 'consistency-api-file-management-routes',
    packageName: 'api-server',
    filter: 'file_management_routes',
  },
];
// Protects issue #916: image_llm routed media retry guidance, text-model fallback,
// and multimodal media preservation. Do not weaken, remove, or rename this gate
// without explicit user consent.
const IMAGE_LLM_VISION_GATE_TARGETS = [
  {
    label: 'cargo-test-image-llm-vision-control-plane-user-media-fallback',
    packageName: 'control-plane',
    filter: 'orchestration_runtime_textualizes_user_media_when_selected_model_is_not_multimodal',
  },
  {
    label: 'cargo-test-image-llm-vision-control-plane-multimodal-preserves-media',
    packageName: 'control-plane',
    filter: 'orchestration_runtime_keeps_user_media_when_configured_model_supports_multimodal',
  },
  {
    label: 'cargo-test-image-llm-vision-control-plane-routed-media-guidance',
    packageName: 'control-plane',
    filter: 'orchestration_runtime_textualizes_routed_media_as_retry_guidance_for_text_models',
  },
  {
    label: 'cargo-test-image-llm-vision-runtime-visible-media-tool',
    packageName: 'orchestration-runtime',
    filter: 'visible_internal_llm_tool_media',
  },
];

// Protects the official i18n bootstrap artifact and its frozen consumer provenance.
// These tests are database-free so they can block pull requests without the full API test shard.
const OFFICIAL_I18N_SEED_GATE_TARGETS = [
  {
    label: 'cargo-test-official-i18n-seed-integrity',
    packageName: 'api-server',
    filter: 'ac_002_decodes_digest_verified_build_time_official_seed',
  },
  {
    label: 'cargo-test-official-i18n-seed-consumer-provenance',
    packageName: 'api-server',
    filter: 'ac_001_ac_002_ac_006_ac_010_ac_012_official_seed_covers_frozen_consumers',
  },
];

module.exports = {
  BACKEND_CONSISTENCY_TARGETS,
  BACKEND_CI_TEST_SHARDS,
  BACKEND_SHARDS,
  BACKEND_TEST_SHARDS,
  IMAGE_LLM_VISION_GATE_TARGETS,
  OFFICIAL_I18N_SEED_GATE_TARGETS,
};
