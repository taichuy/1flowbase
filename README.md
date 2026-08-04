# 1flowbase

<p align="center">
  <img src="docs/assets/logo_index_en.png" alt="1flowbase Logo">
</p>

<p align="center">
  <b>English</b> | <a href="docs/READEME-i18n/README_CN.md">简体中文</a>
</p>

<p align="center">
  <a href="https://github.com/taichuy/1flowbase/stargazers"><img src="https://img.shields.io/github/stars/taichuy/1flowbase?style=social" alt="GitHub stars"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/taichuy/1flowbase" alt="License"></a>
  <img src="https://img.shields.io/badge/OpenAI-compatible-111827" alt="OpenAI compatible">
  <img src="https://img.shields.io/badge/Claude-compatible-111827" alt="Claude compatible">
  <img src="https://img.shields.io/badge/MCP-gateway-7c3aed" alt="MCP gateway">
  <img src="https://img.shields.io/badge/Application_Backend-CRUD-0f766e" alt="Application Backend with CRUD APIs">
  <img src="https://img.shields.io/badge/Native_React-blocks-149eca" alt="Native React blocks">
  <img src="https://img.shields.io/badge/self--hosted-1flowbase-2563eb" alt="Self-hosted">
</p>

<p align="center">
  <strong>Community:</strong>
  <a href="docs/assets/community/wechat.jpg" target="_blank">WeChat</a> |
  <a href="docs/assets/community/taichuy_doc_wechat_office.png" target="_blank">WeChat Official Account</a> |
  <a href="https://x.com/Tacihu2021" target="_blank">Twitter</a>
</p>

> **Build full-stack agent-native applications on four open-source foundations: an AI Gateway, an MCP Gateway, a built-in Application Backend, and Native React frontend blocks.**

1flowbase gives local agents, model clients, external systems, and people independent ways to use the same self-hosted application platform. A local agent can connect only to the MCP Gateway to build and operate applications with its existing model setup. Connecting that client to the AI Gateway is optional and adds compatible model endpoints, multi-model workflows, and detailed execution traces.

```text
Local agents     -> MCP Gateway         -> discover / configure / build / operate
Model clients    -> AI Gateway          -> compatible endpoints / model workflows / traces
External systems -> Application Backend -> generated CRUD APIs / custom workflow APIs
People           -> React blocks        -> interactive application UI

Use any foundation independently, or combine them around one 1flowbase application.
```

| Foundation | What it provides |
|---|---|
| **AI Gateway** | Translate and dispatch OpenAI Responses, Chat Completions, and Claude Messages traffic; route models and publish observable workflows as virtual models |
| **MCP Gateway** | Turn 1flowbase capabilities into progressively discoverable tools; organize tools, mappings, groups, bindings, policies, upstream MCP connections, and reusable bundles |
| **Application Backend** | Define Data Models that materialize PostgreSQL tables, fields, indexes, and relations; generate governed CRUD APIs and publish custom workflow-powered endpoints |
| **Native React frontend blocks** | Build responsive application interfaces with standard React/TSX and CSS, controlled component imports, data binding, and Shadow DOM isolation |

For example, a local agent can use MCP to create `Customer` and `Ticket` Data Models, assemble a workflow-backed `/api/ex/tickets/escalate` operation, and build the React interface. External systems call the generated backend APIs, while people work in the interface. If the same local agent also points its model endpoint at the AI Gateway, it gains published virtual models with routing, model composition, and full logs; the application does not depend on that optional connection.

![Workflow Editor Preview](docs/assets/workflow_editor_preview_tool.png)

---

## What You Can Build

### Let an agent build and operate a 1flowbase application

The MCP Gateway projects platform capabilities into an agent-oriented virtual UI. An agent can progressively discover the relevant domain, inspect a tool contract, call it, verify the resulting state, and continue building without requiring a new hard-coded frontend flow for every task.

```text
Agent
  -> mcp.list: discover applications and capabilities
  -> mcp.get: inspect the next tool contract
  -> mcp.call: create, configure, run, and publish
  -> inspect state / traces
  -> iterate
```

### Create a ready-to-use application backend

Define a Data Model in 1flowbase and publish it. The platform materializes the PostgreSQL schema and exposes model-aware List, Get, Create, Update, and Delete APIs with generated OpenAPI contracts. Use Workflow Extensions when CRUD is not enough and publish custom business operations under `/api/ex/{slug}`.

```text
Data Model definition
  -> PostgreSQL table / columns / indexes / relations
  -> generated CRUD runtime APIs + OpenAPI

Workflow
  -> custom input and output contract
  -> published /api/ex/{slug} operation
```

### Build the human interface with Native React blocks

Write frontend blocks with standard React/TSX, Hooks, events, and CSS. 1flowbase compiles and mounts each block in an isolated Shadow DOM runtime, while controlled catalogs and context contracts expose the platform capabilities the block is allowed to use.

```tsx
export default function StatusCard({ ctx }) {
  const status = ctx.inputs.status;
  return <button onClick={() => ctx.outputs.publish({ action: 'retry' })}>
    {status}
  </button>;
}
```

### Add vision to text-first coding models

Keep GLM-5.2, DeepSeek, or another strong text coding model as the main planner, then let 1flowbase route screenshots, UI images, charts, and PDF pages to a mounted vision model.

```text
Claude Code
  -> 1flowbase virtual model endpoint
  -> GLM-5.2 / DeepSeek / other main coding model
  -> mounted vision tool
  -> GLM-5V-Turbo / Gemini / GPT vision / OCR model
  -> structured visual result
  -> final coding answer
```

Guide: [Make GLM-5.2 See Images in Claude Code with 1flowbase](https://github.com/taichuy/1flowbase/wiki/Make-GLM-5.2-See-Images-in-Claude-Code-with-1flowbase)

### Publish a Fusion-style multi-model reviewer

1flowbase includes a `fusion` template. Your client calls one model name; 1flowbase asks several branch models, runs a synthesis model, returns the final answer, and keeps every branch visible.

```text
User request
  -> Main LLM
  -> fusion tool
     -> Branch LLM A
     -> Branch LLM B
     -> Branch LLM C
     -> Synthesis LLM
  -> final answer
```

Guide: [Fusion-Style Workflows: Publish a Multi-Model Panel as an Observable Virtual Model](https://github.com/taichuy/1flowbase/wiki/Fusion-Style-Workflow)

### Publish workflow-backed model APIs

Build the workflow once, then expose it through common model APIs:

| Protocol | API path | Typical usage |
|---|---:|---|
| OpenAI Responses API | `/v1/responses` | newer OpenAI-style clients and application code |
| OpenAI Chat Completions API | `/v1/chat/completions` | SDKs, coding tools, chat clients, application frameworks |
| Claude-compatible Messages API | `/v1/messages` | Claude-compatible clients that support custom endpoints |

---

## Installation or Upgrade

Linux/macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/taichuy/1flowbase/main/scripts/shell/docker-deploy.sh | sh
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/taichuy/1flowbase/main/scripts/powershell/docker-deploy.ps1 | iex
```

Windows CMD:

```cmd
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/taichuy/1flowbase/main/scripts/powershell/docker-deploy.ps1 | iex"
```

---

## Run From Source

Use this path when you want to develop 1flowbase itself.

Requirements: Node.js `>= 24.0.0`, pnpm, latest stable Rust, and Docker for local middleware.

```bash
git clone https://github.com/taichuy/1flowbase.git
cd 1flowbase

docker compose -f docker/docker-compose.middleware.yaml up -d

cd web
pnpm install
pnpm dev
```

Frontend:

```text
http://127.0.0.1:3100
```

Start backend services:

```bash
cd api
# Copy api/apps/api-server/.env.example to .env before the first run.
cargo run -p api-server --bin api-server
cargo run -p plugin-runner --bin plugin-runner
```

Default backend endpoints:

```text
API Server: http://127.0.0.1:7800
Plugin Runner: http://127.0.0.1:7801
```

Script-assisted startup:

```bash
node scripts/node/dev-up.js
node scripts/node/dev-up.js status
node scripts/node/dev-up.js stop
node scripts/node/dev-up.js restart
```

See [scripts/README.md](scripts/README.md) for more options.

---

## Where 1flowbase Fits

1flowbase is not just a model proxy, a bag of MCP servers, or another chat UI.

| Tool category | What it usually does | How 1flowbase is different |
|---|---|---|
| LLM gateway / model router | routes one request to one provider or model | composes multiple model and tool nodes into one workflow-backed virtual model |
| MCP server / gateway | exposes or aggregates tools for agents | exposes the 1flowbase application control plane while also connecting and governing upstream MCP tools |
| Backend-as-a-Service / backend builder | provides tables and generic CRUD endpoints | materializes governed PostgreSQL Data Models and combines generated CRUD with workflow-powered custom APIs |
| AI workflow builder | builds an AI app or workflow | exposes the workflow through model APIs and makes the surrounding application operable through MCP |
| Agent application framework | helps developers code agent graphs and interfaces | combines a visual runtime, agent-facing control plane, protocol publishing, and Native React application surfaces |
| Observability / cost tracker | shows token or spend totals | connects cost to workflow nodes, model calls, tool callbacks, and trace logs |

```text
Model serving plane: AI Gateway (optional for external clients)
Agent control plane: MCP Gateway
Data and API plane: Application Backend
Human experience plane: Native React frontend blocks

1flowbase provides all four foundations in one self-hosted platform.
```

---

## Feature Preview

### Publish as OpenAI-compatible API

![Publish OpenAI API](docs/assets/api_endpoint_publish_1.jpeg)

### Publish as Claude-compatible Messages API

![Publish Claude API](docs/assets/api_endpoint_publish_2.jpeg)

### Customize exposed model information

![Custom Model Settings](docs/assets/custom_model_settings.jpeg)

### Use in local AI agent clients

Call a published workflow from compatible clients that support custom model endpoints.

![Claude Code Terminal Usage](docs/assets/claude_code_terminal_usage.png)

### Inspect execution logs

Trace model requests, node inputs and outputs, tool callbacks, response content, latency, and errors.

![Detailed Execution Logs](docs/assets/detailed_execution_logs.jpeg)

### View tool callback traces

![Tool Callback Trace Logs](docs/assets/tool_callback_trace_logs.png)

### Track token consumption

![Token Consumption Dashboard](docs/assets/token_consumption_dashboard.jpeg)

---

## Common Use Cases

### Make a text coding model understand screenshots

```text
Screenshot / UI mockup / chart
  -> vision tool
  -> structured visual context
  -> strong coding model
  -> patch, plan, or explanation
```

Useful for UI reconstruction, frontend debugging, visual regression analysis, chart reading, PDF page understanding, and design-to-code workflows.

### Build a Fusion-style reviewer

```text
Architecture proposal
  -> cheap broad reviewer
  -> strong reasoning reviewer
  -> provider-diverse reviewer
  -> synthesis model
  -> final recommendation
```

Useful for architecture review, research synthesis, code review, document review, and high-stakes agent decisions.

### Control cost with model cascading

```text
Simple classification -> small model
Formatting -> small model
Complex reasoning -> strong model
Final verification -> verifier node
```

### Guarantee output structure

Use verifiers, JSON Schema validation, and formatter nodes before returning the final result. This is useful for JSON outputs, API responses, tool call parameters, code patches, document generation, and automated task results.

### Build a programmable upstream model for agents

```text
Code generation -> test / lint check -> reviewer node -> fix node -> final patch
```

The client calls one model name while 1flowbase runs your workflow behind it.

---

## Current Status

### Implemented

- [x] visual workflow editor
- [x] multi-node workflow orchestration
- [x] virtual model endpoint publishing
- [x] OpenAI Responses protocol support
- [x] OpenAI Chat Completions protocol support
- [x] Claude-compatible Messages protocol support
- [x] streaming response support
- [x] mounted LLM tools for multimodal and branch-model workflows
- [x] `fusion` workflow template
- [x] execution logs
- [x] tool callback traces inside 1flowbase workflows
- [x] application-level token consumption statistics
- [x] prompt and model configuration version history
- [x] MCP Gateway with progressive `mcp.list`, `mcp.get`, and `mcp.call` discovery
- [x] MCP tool, mapping, group, binding, discovery policy, and upstream connection management
- [x] reusable MCP bundle import, export, validation, and local library flows
- [x] dynamic Data Models that materialize PostgreSQL tables, fields, indexes, and relations
- [x] generated Data Model CRUD runtime APIs and OpenAPI contracts
- [x] custom Workflow Extension APIs published under `/api/ex/{slug}`
- [x] Native React/TSX frontend blocks with Hooks, CSS, events, and Shadow DOM isolation
- [x] controlled frontend component catalog, code studio, responsive context, and data binding

### Enhancing

- [ ] deeper local agent conversation collection
- [ ] session search and playback
- [ ] Token Bill of Materials by prompt, history, tool definitions, command outputs, media inputs, and nodes
- [ ] abnormal cost detection and optimization suggestions
- [ ] session export and Recall Pack generation
- [ ] more Claude Code / Codex / OpenCode / Cline / Continue templates
- [ ] broader agent-ready MCP Virtual UI coverage across product domains
- [ ] more end-to-end application recipes combining MCP management, Application Backend, frontend blocks, and optional AI Gateway serving

### Planned

- [ ] broader low-code authoring on top of the four application foundations
- [ ] team workspace and multi-tenant management
- [ ] permissions, approval, audit, and cost governance
- [ ] support for more local AI agent clients
- [ ] template market and workflow recipe ecosystem

> The four foundations are implemented and usable today. They are composable rather than mandatory stages: an external agent can use the MCP Gateway without routing its model traffic through the AI Gateway. Broader low-code authoring, team governance, and the template ecosystem are still evolving.

---

## Transparency and Security

1flowbase is designed for transparent, self-hosted AI workflow execution.

Recommended principles:

- self-hosted first
- transparent model chains
- auditable node calls
- traceable token usage
- configurable log retention
- sensitive data masking
- explicit model and workflow configuration

1flowbase does not advocate stealthy model replacement. Published endpoints should be configured intentionally, observed clearly, and governed by the project owner.

---

## Guides

- [Make GLM-5.2 See Images in Claude Code with 1flowbase](https://github.com/taichuy/1flowbase/wiki/Make-GLM-5.2-See-Images-in-Claude-Code-with-1flowbase)
- [Fusion-Style Workflows: Publish a Multi-Model Panel as an Observable Virtual Model](https://github.com/taichuy/1flowbase/wiki/Fusion-Style-Workflow)
- [1flowbase Wiki](https://github.com/taichuy/1flowbase/wiki)

---

## Repo Layout

```text
web/          Frontend root, powered by pnpm + Turbo
api/          Rust backend workspace
api/apps/     Backend service entry points
api/crates/   Shared backend crates
api/plugins/  Plugin workspace, HostExtension manifests, and templates
docker/       Local middleware orchestration and self-hosted stack
scripts/      Development, testing, verification, and debugging scripts
```

---

## Contributing

Contributions are welcome. Before submitting a pull request, run:

```bash
node scripts/node/verify.js repo
```

Project guidelines:

- [AGENTS.md](AGENTS.md)
- [web/AGENTS.md](web/AGENTS.md)
- [api/AGENTS.md](api/AGENTS.md)

---

## Friend Links

- [Linux.do](https://linux.do/) - Learn AI, on L Station.
- [Aionui](https://github.com/iOfficeAI/AionUi) - Remotely control AI to work via mobile phone.
- [OfficeCLI](https://github.com/iOfficeAI/OfficeCLI) - Office suite designed for AI agents.
- [deepseek-pp](https://github.com/zhu1090093659/deepseek-pp) - DeepSeek web chat browser extension.
- [MuseAI](https://github.com/yejiming/MuseAI) - Local AI companion, text adventure, and story immersion app.
- [FrontAgent](https://github.com/FrontAgent/FrontAgent) - AI Agent system designed specifically for front-end engineering.
- [RedBox](https://github.com/Jamailar/RedBox) - Localized AI creative workbench for Xiaohongshu creators.

---

## OpenAI Build Week

### Built with Codex and GPT-5.6

Codex was used to:
- redesign MCP configuration workflows
- implement and review Rust backend changes
- improve React components
- debug integration issues

GPT-5.6 was used for:
- designing agent workflows
- validating MCP capability patterns
- improving architecture decisions

## License

This project is licensed under [Apache-2.0](LICENSE).

---

## Contributors

<p align="center">
  <a href="https://github.com/taichuy/1flowbase/graphs/contributors">
    <img src="https://contrib.rocks/image?repo=taichuy/1flowbase&max=50" alt="Contributors" />
  </a>
</p>

---

## Star History

## Star History

<a href="https://www.star-history.com/?type=date&repos=taichuy%2F1flowbase">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=taichuy/1flowbase&type=date&theme=dark&legend=top-left&sealed_token=FqLzVSU8-9DxFglG-qgV59WwozJJfOHYwvjWNeVtnDP8OJ8r8BwvdLCIloKkdrLXWJqUEaD9xkVSr0RkCvzGaIxDYXYX2Zz53ikx7xZkZckNqgevZkOi1A" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=taichuy/1flowbase&type=date&legend=top-left&sealed_token=FqLzVSU8-9DxFglG-qgV59WwozJJfOHYwvjWNeVtnDP8OJ8r8BwvdLCIloKkdrLXWJqUEaD9xkVSr0RkCvzGaIxDYXYX2Zz53ikx7xZkZckNqgevZkOi1A" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=taichuy/1flowbase&type=date&legend=top-left&sealed_token=FqLzVSU8-9DxFglG-qgV59WwozJJfOHYwvjWNeVtnDP8OJ8r8BwvdLCIloKkdrLXWJqUEaD9xkVSr0RkCvzGaIxDYXYX2Zz53ikx7xZkZckNqgevZkOi1A" />
 </picture>
</a>

---

<div align="center">

**If you want agents to build and operate self-hosted applications across AI, MCP, backend, and React surfaces, give 1flowbase a star.**

[Report Bug](https://github.com/taichuy/1flowbase/issues) · [Request Feature](https://github.com/taichuy/1flowbase/issues)

</div>
