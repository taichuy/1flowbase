'use strict';

const clientLogFilter = 'warn,codex_core=info,codex_core::session::turn=trace,codex_api::endpoint::responses_websocket=debug,feedback_tags=info,codex_otel.agent_communication=trace';

function retainStderrLine(line) {
  const text = line.replace(/\x1b\[[0-9;]*m/g, '');
  const level = text.match(/^\S+Z\s+(TRACE|DEBUG|INFO|WARN|ERROR)\b/)?.[1];
  if (!level || level === 'WARN' || level === 'ERROR') return true;
  if (/sampling_error=|Turn error:|post sampling token usage|codex_otel\.agent_communication:/.test(text)) return true;
  return text.includes('codex_api::endpoint::responses_websocket:')
    && (/successfully connected to websocket:/.test(text) || /responses_websocket: (?:new|close(?:\s.*)?)$/.test(text));
}

function clientConfig({ catalogPath, baseUrl }) {
  return `model = "gpt-6-luna"
model_provider = "candidate"
model_reasoning_effort = "max"
model_catalog_json = ${JSON.stringify(catalogPath)}
approval_policy = "never"
sandbox_mode = "read-only"
[features]
multi_agent = true
[model_providers.candidate]
name = "Issue 2175 candidate"
base_url = ${JSON.stringify(baseUrl)}
env_key = "RLS_GATEWAY_KEY"
wire_api = "responses"
supports_websockets = true
`;
}

module.exports = { clientConfig, clientLogFilter, retainStderrLine };
