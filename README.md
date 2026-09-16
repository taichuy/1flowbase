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
list
 │
 ▼
Discover available Tools

get
 │
 ▼
Get full Tool definition

call
 │
 ▼
Execute Tool
```

Agents only need to understand three basic tools:

**list / get / call**

Then progressively discover and invoke the capabilities they actually need based on the current task.

This avoids stuffing a massive number of Tool definitions into the context all at once.

------

# React Blocks

Business systems ultimately still need UI.

1flowbase provides **React**-based code block capabilities.

Each Block can directly contain React code, so you're not limited by fixed low-code components.

For example, Ant Design's official example code can serve directly as the foundation for a Block:

```jsx
import React from 'react';
import { ColorPicker, Space } from 'antd';

const DEFAULT_COLOR = [
  {
    color: 'rgb(16, 142, 233)',
    percent: 0,
  },
  {
    color: 'rgb(135, 208, 104)',
    percent: 100,
  },
];

const Demo = () => (
  <Space vertical>
    <ColorPicker
      defaultValue={DEFAULT_COLOR}
      allowClear
      showText
      mode={['single', 'gradient']}
      onChangeComplete={(color) => {
        console.log(color.toCssString());
      }}
    />
  </Space>
);

export default Demo;
```

You can continue using the entire React ecosystem to build your business interfaces.

So 1flowbase's goal isn't to provide a closed drag-and-drop page editor.

It's to:

> **Strike a balance between low-code efficiency and native React freedom.**

------

# Built for Agents

We're entering a new software era:

> **Build for Agents.**

But many systems today that claim to be "built for Agents" are really just adding a few Tools for Agents.

The infrastructure itself is still primarily designed for humans:

```text
Human
  │
  ▼
Dashboard
  │
  ▼
Buttons / Forms
  │
  ▼
Application
```

1flowbase takes a different approach.

If the GUI can perform a runtime operation, that capability should also be exposable through an API.

And APIs can further be exposed to Agents through MCP.

```text
           Human
             │
             ▼
            GUI
             │
             ▼
            API
             ▲
             │
            MCP
             ▲
             │
           Agent
```

So humans and Agents operate the same system.

In 1flowbase's runtime, Agents can further participate in:

- Creating and modifying Gateways
- Configuring models
- Creating Workflows
- Creating data tables
- Managing business data
- Creating and calling APIs
- Querying runtime logs
- Configuring permissions
- Building React interfaces

So the problem we want to solve isn't:

> **How do you add an Agent to an application?**

It's:

> **How do you build an application system truly built for Agents?**

------

# From AI Gateway to Full Application System

Ultimately, you can combine all these capabilities:

```text
AI Gateway
     +
Workflow
     +
API
     +
Business Data
     +
React UI
     +
MCP
     │
     ▼
AI Application
```

In other words:

> **Build a complete application system truly built for Agents on top of an AI Gateway.**

------

# What Can an Agent Do?

For example, connecting 1flowbase MCP to Codex:

```text
Codex
  │
  ▼
MCP
  │
  ▼
1flowbase
```

Agents can continue operating 1flowbase based on your business needs:

```text
Create Data Model
       ↓
Create Business API
       ↓
Configure AI Gateway
       ↓
Create Workflow
       ↓
Build React UI
       ↓
Operate Application
```

## Current Status

1flowbase's underlying runtime is already built API-first, so these capabilities can be further exposed to MCP.

We're currently actively improving:

- Official MCP Tool definitions
- Tool descriptions
- Agent Context
- Default configurations
- Application templates

The goal is to enable Coding Agents like Codex to **understand platform capabilities and build applications directly without reading 1flowbase source code**.

At the current stage, for complex application building scenarios, we still recommend having Coding Agents work within the 1flowbase project context.

------

# Out-of-the-Box Application Templates

1flowbase itself has many capabilities.

This is also one of the issues we're currently focused on improving:

> **A powerful system shouldn't require every new user to start from a blank canvas.**

So we're packaging common capabilities into application templates.

For example, in the future you'll be able to start directly from:

```text
AI Gateway
Multi-model Router
Enterprise AI Gateway
AI Education Platform
Agent Application Backend
```

And other scenarios.

You can use a template to handle 80% of the basic configuration, then extend further based on your business needs.

------

# Quick Start

## Linux / macOS

Run the official deployment script:

```bash
curl -fsSL https://raw.githubusercontent.com/taichuy/1flowbase/main/scripts/shell/docker-deploy.sh | sh
```

Follow the prompts to complete configuration and start 1flowbase.

------

## Windows PowerShell

```powershell
irm https://raw.githubusercontent.com/taichuy/1flowbase/main/scripts/powershell/docker-deploy.ps1 | iex
```

------

## Windows CMD

```cmd
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/taichuy/1flowbase/main/scripts/powershell/docker-deploy.ps1 | iex"
```

The entire process uses Docker deployment — no need to manually set up complex runtime environments.

------

# Who Is It For?

## Enterprise AI Teams

If your organization needs unified distribution of:

- OpenAI
- Anthropic
- Other model providers
- Custom models
- Composite models
- Agent Workflows

And you also want to:

- Own your AI data
- Analyze enterprise AI usage behavior
- Build internal AI applications
- Provide unified AI capabilities across departments

1flowbase can serve as this infrastructure layer.

------

## AI Product Teams

If you're building AI SaaS or Agent products, you can start from the Gateway and expand:

```text
Gateway → Data → API → Workflow → UI
```

Without having to build multiple disconnected systems separately.

------

## Advanced Individual Users

If you use multiple models simultaneously, you can also leverage Workflows to build your own composite models:

> High-capability models handle decisions, low-cost models handle execution.

Then use it as a single model through a unified API.

------

# How Is 1flowbase Different from Other Tools?

| Project    | AI Gateway | Composite Models / Workflow | Full Conversation Data | Dynamic Business Data | MCP      | React Apps   |
| ---------- | ---------- | --------------------------- | ---------------------- | --------------------- | -------- | ------------ |
| 1flowbase  | ✅          | ✅                           | ✅                      | ✅                     | ✅        | ✅            |
| LiteLLM    | ✅          | —                           | Partial                | —                     | —        | —            |
| OpenRouter | ✅          | —                           | Platform-hosted        | —                     | —        | —            |
| Dify       | Partial    | ✅                           | ✅                      | —                     | ✅        | Fixed app forms |
| Supabase   | —          | —                           | —                      | ✅                     | Ecosystem | Build your own frontend |
| n8n        | —          | ✅                           | —                      | —                     | Ecosystem | —            |

------

# Data Storage

1flowbase currently uses PostgreSQL as its primary data source.

Since storing complete AI conversations can generate significant data volume, for high-traffic production environments, please evaluate in advance:

- Storage capacity
- Data retention periods
- Backup strategies
- Compliance requirements

We also plan to continue strengthening AI data lifecycle management capabilities.

------

# Roadmap

Current priorities include:

- Simpler default AI Gateway configuration
- Common model routing templates
- Composite model templates
- Enterprise AI Gateway templates
- Improved MCP Tool descriptions
- Better understanding of system capabilities by Coding Agents like Codex
- Reduced dependency on project source context when Agents build applications
- Improved AI conversation data management and lifecycle policies
- More out-of-the-box business application templates


---

# Star 1flowbase

If you also believe the future of software isn't just:

> **Built with AI**

But should go further:

> **Built for Agents**

Give 1flowbase a ⭐.

What we want to explore is:

> **When AI Gateway, business data, APIs, Workflows, and UI can all be operated by Agents — what should applications become?**

---

## Tutorials

- [Make GLM-5.2 See Images in Claude Code with 1flowbase](https://github.com/taichuy/1flowbase/wiki/Make-GLM-5.2-See-Images-in-Claude-Code-with-1flowbase-CN)
- [Fusion-Style Workflow: Publish a Multi-Model Review Panel as an Observable Virtual Model](https://github.com/taichuy/1flowbase/wiki/Fusion-Style-Workflow-CN)
- [1flowbase Wiki](https://github.com/taichuy/1flowbase/wiki)

---

## Repository Layout

```text
web/          Frontend root, powered by pnpm + Turbo
api/          Rust backend workspace
api/apps/     Backend service entry points
api/crates/   Shared backend crates
api/plugins/  Plugin source workspace, HostExtension manifests and templates
docker/       Local middleware orchestration and self-hosted service stack
scripts/      Repository-level development, testing, verification, and debugging scripts
```

---

## Contributing

Community contributions are very welcome. Before submitting a Pull Request, please run the following verification script:

```bash
node scripts/node/verify.js repo
```

Development guidelines:

- [AGENTS.md](../../AGENTS.md)
- [web/AGENTS.md](../../web/AGENTS.md)
- [api/AGENTS.md](../../api/AGENTS.md)

---

## Friends

- [Linux.do](https://linux.do/) - Learn AI, on L Station.
- [Aionui](https://github.com/iOfficeAI/AionUi) - Remotely control AI from your phone.
- [OfficeCLI](https://github.com/iOfficeAI/OfficeCLI) - Office suite designed for AI Agents.
- [deepseek-pp](https://github.com/zhu1090093659/deepseek-pp) - DeepSeek web chat browser extension.
- [MuseAI](https://github.com/yejiming/MuseAI) - Local AI companion, text adventure, and interactive fiction app.
- [FrontAgent](https://github.com/FrontAgent/FrontAgent) - AI Agent system designed for frontend engineering.
- [RedBox](https://github.com/Jamailar/RedBox) - Localized AI creation workspace for Xiaohongshu creators.

---

## License

This project is licensed under [Apache-2.0](./LICENSE).

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

**If you want Agents to build and operate self-hosted applications across AI, MCP, application backends, and React interfaces — give 1flowbase a Star.**

[Report a Bug](https://github.com/taichuy/1flowbase/issues) · [Request a Feature](https://github.com/taichuy/1flowbase/issues)

</div>