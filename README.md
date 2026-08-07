# 1flowbase

<p align="center">
  <img src="web/app/public/icon.svg" alt="1flowbase Logo" width="120" height="120">
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

> 1flowbase is a self-hosted AI gateway for individuals and enterprises: on top of protocol translation, dispatch, and detailed chat logs, it ships with a built-in Application Backend and Native React frontend blocks that help you combine AI with your business data. Most importantly, all of it can be operated and managed by your Agent through MCP.

1flowbase lets an Agent take over the entire application through MCP — whether orchestrating and publishing an AI gateway, generating backend application endpoints, or building native React interfaces.

```text
Local Agent        -> MCP Gateway         -> discover / configure / build / operate
Model clients      -> AI Gateway          -> compatible endpoints / model workflows / traces
External systems   -> Application Backend -> generated CRUD APIs / custom workflow APIs
People             -> React blocks        -> interactive application UI

The four foundations can be used independently, or combined around the same 1flowbase application.
```

| Foundation | What it provides |
|---|---|
| **AI Gateway** | Translate and dispatch OpenAI Responses, Chat Completions, and Claude Messages traffic; route models and publish observable workflows as virtual models |
| **MCP Gateway** | Project 1flowbase capabilities into progressively discoverable tools; manage Tools, mappings, Groups, Bindings, policies, upstream MCP connections, and reusable Bundles |
| **Application Backend** | Define Data Models that materialize PostgreSQL tables, fields, indexes, and relations; automatically generate governed CRUD APIs and publish custom endpoints powered by Workflows |
| **Native React frontend blocks** | Build responsive application interfaces with standard React/TSX and CSS, controlled component imports, data binding, and Shadow DOM isolation |

For example, a local Agent can create `Customer` and `Ticket` Data Models through MCP, assemble a workflow-backed `/api/ex/tickets/escalate` endpoint, and build the React interface. External systems call the generated backend APIs, while people work directly in the interface. If the same local Agent also points its model endpoint at the AI Gateway, it gains virtual models with routing, model composition, and full logs; the application itself does not depend on this optional connection.

---

## What You Can Build

### Let an agent build and operate a 1flowbase application

The MCP Gateway projects platform capabilities into an agent-oriented virtual UI. An agent can progressively discover the relevant domain, inspect a tool contract, make a call, verify the resulting state, and continue building — no hard-coded frontend flow needed for each new task.

```text
Agent
  -> mcp_list: discover applications and capabilities
  -> mcp_get: inspect the next tool contract
  -> mcp_call: create, configure, run, and publish
  -> inspect state / traces
  -> iterate
```

### Create a ready-to-use application backend

Define and publish a Data Model in 1flowbase, and the platform materializes the PostgreSQL schema and generates model-aware List, Get, Create, Update, and Delete APIs with OpenAPI contracts. When standard CRUD is not enough, use a Workflow Extension to define business logic and publish it as a custom endpoint under `/api/ex/{slug}`.

```text
Data Model definition
  -> PostgreSQL table / columns / indexes / relations
  -> generated CRUD runtime APIs + OpenAPI

Workflow
  -> custom input/output contract
  -> published /api/ex/{slug} endpoint
```

### Build the human interface with Native React blocks

Frontend blocks use standard React/TSX, Hooks, events, and CSS directly. 1flowbase compiles and mounts each block in an isolated Shadow DOM runtime, exposing the platform capabilities the block is allowed to use through controlled catalogs and context contracts.

```tsx
export default function StatusCard({ ctx }) {
  const status = ctx.inputs.status;
  return <button onClick={() => ctx.outputs.publish({ action: 'retry' })}>
    {status}
  </button>;
}
```

### Add vision to text-first coding models

Keep GLM-5.2, DeepSeek V4, or another strong text-based coding model in charge of planning and writing code, and let 1flowbase route screenshots, UI images, charts, and PDF pages to a mounted vision model.

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

1flowbase ships with a `fusion` template. The client calls a single model name; 1flowbase queries multiple branch models in the background, invokes a synthesis model, returns the final answer, and keeps the execution record of every branch.

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

Build the workflow once, then serve it through common model protocols:

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

This path is for developing 1flowbase itself.

Requirements: Node.js `>= 24.0.0`, pnpm, the latest stable Rust, and Docker for local middleware.

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

See [scripts/README.md](scripts/README.md) for more configuration options.

---

## Common Use Cases

### Have an agent build and continuously manage an internal application

```text
Local or external Agent
  -> MCP Gateway
  -> create Data Models and relations
  -> publish CRUD and Workflow Extension APIs
  -> assemble Native React blocks
  -> inspect and continuously evolve the running application
```

This is the primary full-stack path formed by the four foundations: the agent operates the control plane through MCP, the Application Backend handles data and APIs, and Native React blocks provide the human interface. The AI Gateway is connected on demand only when the application also needs to serve governed model endpoints externally.

### Deliver an application backend without assembling a separate backend stack

```text
Data Model
  -> PostgreSQL table / columns / indexes / relations
  -> generated CRUD runtime and OpenAPI
  -> custom business logic via Workflow Extension APIs
```

Ideal for internal tools, management systems, operations dashboards, agent memory storage, content systems, and small-to-medium product backends.

### Add custom human interfaces to agent-managed data

```text
Data Model / custom APIs
  -> Native React blocks
  -> search, filters, forms, actions, and responsive layouts
```

The task planning board is a real example: the native React interface reads and updates records directly through the Data Model API, without a separate frontend-backend stack.

### Publish a programmable upstream model for AI clients

```text
External AI clients
  -> optional AI Gateway
  -> protocol translation
  -> model and tool workflows
  -> logs, traces, token usage, and final responses
```

The client calls a single model name, and 1flowbase can run cross-provider workflows behind it. Suitable for multimodal enhancement, Fusion-style review, model cascading, structured output validation, and programmable coding model flows.

---

## Transparency and Security

1flowbase is committed to providing a transparent, self-hosted environment for AI workflow execution.

Recommended principles:

- self-hosted first
- transparent model chains
- auditable node calls
- traceable token usage
- configurable log retention
- explicit model and workflow configuration

1flowbase does not advocate silently replacing models without the user's knowledge. Every published endpoint should be clearly configured, observed, and governed by the project owner.

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

Community contributions are very welcome. Before submitting a pull request, run the following verification script:

```bash
node scripts/node/verify.js repo
```

Project development guidelines:

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

## License

This project is licensed under the [Apache-2.0](LICENSE) open-source license.

---

## Contributors

<p align="center">
  <a href="https://github.com/taichuy/1flowbase/graphs/contributors">
    <img src="https://contrib.rocks/image?repo=taichuy/1flowbase&max=50" alt="Contributors" />
  </a>
</p>

---

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

**If you want agents to build and operate self-hosted applications across AI, MCP, Application Backend, and React surfaces, give 1flowbase a star.**

[Report Bug](https://github.com/taichuy/1flowbase/issues) · [Request Feature](https://github.com/taichuy/1flowbase/issues)

</div>
