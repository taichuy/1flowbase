'use strict';

const crypto = require('node:crypto');

function requireData(value, label) {
  if (!value || typeof value !== 'object') throw new Error(`${label} response omitted data`);
  return value;
}

async function installProvider(client, archivePath, expectedProviderCode) {
  const uploaded = await client.uploadPackage(archivePath);
  const installation = requireData(uploaded.installation, 'package upload installation');
  if (installation.provider_code !== expectedProviderCode) {
    throw new Error(`expected ${expectedProviderCode} package, received ${installation.provider_code || 'unknown'}`);
  }
  const installationId = installation.id;
  await client.write(`/api/console/plugins/${installationId}/enable`);
  await client.write(`/api/console/plugins/${installationId}/assign`);
  return {
    installation_id: installationId,
    provider_code: expectedProviderCode,
    package_sha256: uploaded.archive_sha256,
  };
}

async function createProviderInstance(client, installation, upstreamBaseUrl, model, ordinal = 1) {
  const providerBaseUrl = installation.provider_code === 'openai'
    ? `${upstreamBaseUrl}/v1`
    : upstreamBaseUrl;
  const config = {
    base_url: providerBaseUrl,
    api_key: `fixture-${installation.provider_code}-token`,
    validate_model: false,
  };
  if (installation.provider_code === 'openai') config.transport_mode = 'http_sse';
  const { data } = await client.write(
    '/api/console/settings/model-providers/instances',
    'POST',
    {
      installation_id: installation.installation_id,
      display_name: `Gateway fixture ${installation.provider_code} ${ordinal}`,
      configured_models: [{
        model_id: model,
        enabled: true,
        context_window_override_tokens: null,
        supports_multimodal: false,
      }],
      enabled_model_ids: [model],
      included_in_main: true,
      preview_token: null,
      config,
    }
  );
  if (typeof data?.id !== 'string') throw new Error('provider instance response omitted id');
  return { ...installation, provider_instance_id: data.id, model };
}

function configureDraft(document, provider) {
  const nodes = document?.graph?.nodes;
  if (!Array.isArray(nodes)) throw new Error('application draft omitted graph nodes');
  const start = nodes.find((node) => node?.type === 'start');
  const llm = nodes.find((node) => node?.type === 'llm');
  if (!start || !llm) throw new Error('application draft omitted start or llm node');
  start.config.model_list = [{
    id: provider.model,
    name: provider.model,
    context_window: 128000,
    max_output_tokens: 4096,
    capabilities: {
      reasoning: true,
      tool_call: true,
      multimodal: false,
      structured_output: true,
    },
  }];
  llm.config.model_provider = {
    provider_code: provider.provider_code,
    source_instance_id: provider.provider_instance_id,
    model_id: provider.model,
  };
  return document;
}

async function createPublishedApplication(client, provider, ordinal = 1) {
  const suffix = crypto.randomBytes(5).toString('hex');
  const created = await client.write('/api/console/applications', 'POST', {
    application_type: 'agent_flow',
    workflow_trigger_type: null,
    workflow_trigger_config: null,
    name: `Gateway fixture ${provider.provider_code} ${ordinal} ${suffix}`,
    description: 'Temporary AI gateway concurrency fixture',
    icon: null,
    icon_type: null,
    icon_background: null,
  });
  const applicationId = created.data?.id;
  if (typeof applicationId !== 'string') throw new Error('application response omitted id');

  const key = await client.write(`/api/console/applications/${applicationId}/api-keys`, 'POST', {
    name: `Gateway fixture key ${ordinal}`,
    expires_at: null,
  });
  if (typeof key.data?.token !== 'string' || typeof key.data?.id !== 'string') {
    throw new Error('application API key response omitted id or token');
  }

  const orchestration = await client.read(`/api/console/applications/${applicationId}/orchestration`);
  const document = configureDraft(orchestration.data?.draft?.document, provider);
  await client.write(`/api/console/applications/${applicationId}/orchestration/draft`, 'PUT', {
    document,
    change_kind: 'logical',
    summary: `Configure ${provider.provider_code} loopback provider`,
  });

  const mapping = {
    input: {
      query_target: 'node-start.query',
      model_target: null,
      inputs_target: null,
      history_target: 'node-start.history',
      attachments_target: null,
    },
    output: {
      answer_selector: 'answer',
      usage_selector: null,
      files_selector: null,
      error_selector: null,
    },
  };
  const publication = await client.write(
    `/api/console/applications/${applicationId}/api-publications`,
    'POST',
    { mapping, api_enabled: true }
  );
  const publicationId = publication.data?.id ?? publication.data?.version_id;
  if (typeof publicationId !== 'string' || !publicationId) {
    throw new Error('application publication response omitted id');
  }

  return {
    ...provider,
    application_id: applicationId,
    api_key_id: key.data.id,
    api_key: key.data.token,
    publication_id: publicationId,
  };
}

async function bootstrapGateway(client, options) {
  await client.signIn(options.rootAccount, options.rootPassword);
  const packages = await Promise.all([
    installProvider(client, options.openaiPackage, 'openai'),
    installProvider(client, options.anthropicPackage, 'anthropic'),
  ]);
  const openaiInstallation = packages.find((item) => item.provider_code === 'openai');
  const anthropicInstallation = packages.find((item) => item.provider_code === 'anthropic');
  const openaiInstance = await createProviderInstance(
    client, openaiInstallation, options.upstreamBaseUrl, options.model
  );
  const anthropicInstances = await Promise.all([1, 2].map((ordinal) => createProviderInstance(
    client, anthropicInstallation, options.upstreamBaseUrl, options.model, ordinal
  )));
  return {
    openai: await createPublishedApplication(client, openaiInstance),
    anthropic: await Promise.all(anthropicInstances.map(
      (instance, index) => createPublishedApplication(client, instance, index + 1)
    )),
  };
}

module.exports = { bootstrapGateway, configureDraft, createPublishedApplication, installProvider };
