# 1flowbase

<p align="center">
  <img src="../../web/app/public/icon.svg" alt="1flowbase Logo" width="120" height="120">
</p>

<p align="center">
  <a href="../../README.md">English</a> | <b>简体中文</b>
</p>

<p align="center">
  <a href="https://github.com/taichuy/1flowbase/stargazers"><img src="https://img.shields.io/github/stars/taichuy/1flowbase?style=social" alt="GitHub stars"></a>
  <a href="../../LICENSE"><img src="https://img.shields.io/github/license/taichuy/1flowbase" alt="License"></a>
  <img src="https://img.shields.io/badge/OpenAI-compatible-111827" alt="OpenAI compatible">
  <img src="https://img.shields.io/badge/Claude-compatible-111827" alt="Claude compatible">
  <img src="https://img.shields.io/badge/MCP-gateway-7c3aed" alt="MCP gateway">
  <img src="https://img.shields.io/badge/Application_Backend-CRUD-0f766e" alt="Application Backend with CRUD APIs">
  <img src="https://img.shields.io/badge/Native_React-blocks-149eca" alt="Native React blocks">
  <img src="https://img.shields.io/badge/self--hosted-1flowbase-2563eb" alt="Self-hosted">
</p>

<p align="center">
  <strong>交流与社区：</strong>
  <a href="../assets/community/wechat.jpg" target="_blank">微信</a> |
  <a href="../assets/community/taichuy_doc_wechat_office.png" target="_blank">微信公众号（文档）</a> |
  <a href="https://x.com/Tacihu2021" target="_blank">Twitter</a>
</p>

# 1flowbase

> ## 为 Agent 构建——从 AI Gateway 到完整应用系统。

**1flowbase 是一个可自托管的 AI Gateway 与 AI 应用运行时。**

从统一接入模型开始，你可以组合和分发 AI 能力、完整沉淀会话数据，并继续在这些能力和数据之上构建 **Workflow、API、业务数据与 React 应用**。

更重要的是，1flowbase 的运行时能力以 API 为基础，并可通过 MCP 暴露给 Agent。

**不只是给 Agent 提供几个工具。**

**而是让整个应用系统都能够被 Agent 理解、构建和管理。**

```text
                    Agent
                      │
                     MCP
                      │
        ┌─────────────▼─────────────┐
        │         1flowbase         │
        │                           │
        │  AI Gateway   Workflow    │
        │  Models       API         │
        │  Chat Data    Data Source │
        │  MCP Gateway  React UI    │
        │                           │
        └─────────────┬─────────────┘
                      │
               AI Application
```

**分发智能，沉淀数据，构建应用。**

------

## 为什么是 1flowbase？

大多数 AI Gateway 解决的是：

```text
Application
    │
    ▼
AI Gateway
    │
    ├── OpenAI
    ├── Anthropic
    └── Other Providers
```

统一协议、模型路由和请求分发非常重要。

但对于真正运行 AI 业务的团队来说，**Gateway 不应该是终点。**

每一次 AI 请求都会产生新的业务数据：

- 用户提出了什么问题
- AI 给出了什么答案
- Agent 如何完成任务
- 哪个模型执行了哪部分工作
- 哪些任务失败了
- 哪些任务成本过高
- 用户真正需要什么能力

这些不应该只是转瞬即逝的请求日志。

**它们可以成为企业自己的 AI 数据资产。**

1flowbase 从 AI Gateway 开始，把模型分发、会话数据、工作流、API、业务数据和前端应用放进同一个运行时中。

```text
AI Gateway
    ↓
Conversation Data
    ↓
Workflow / API / Data
    ↓
Business Application
```

因此，你可以从一次模型调用开始，逐渐构建出一个完整的 AI 应用系统。

------

# 一个 Gateway，不一定只是一个模型

在传统 AI Gateway 中，一个模型名称通常只是某个模型供应商的映射。

例如：

```text
my-model
    ↓
Provider
    ↓
Model
```

在 1flowbase 中，一个对外发布的“模型”也可以是一条 **Workflow**。

这意味着你可以把多个模型和工具组合成新的 AI 能力，再通过统一 Gateway 对外发布。

例如：

```text
用户请求
   │
   ▼
高能力模型
规划 / 判断
   │
   ▼
低成本工作模型
执行任务
   │
   ▼
高能力模型
检查 / 审计
   │
   ▼
最终结果
```

高能力模型不一定需要完成整个任务。

很多场景真正需要高能力模型参与的是：

**规划、判断、决策和审计。**

大量具体执行工作，可以交给更快速、更低成本的模型。

这样，你可以把不同模型的：

- 能力
- 成本
- 速度
- 上下文
- 推理质量

组合起来，而不是被单一模型绑定。

------

## 让模型调用模型

1flowbase 还可以把另一个模型作为 Tool 提供给主模型。

```text
Primary Model
      │
      ├── Search
      ├── Database
      ├── Business API
      │
      └── Worker Model
              │
              ▼
        Lower-cost Model
```

主模型可以根据任务决定是否调用工作模型。

工作模型完成任务后，结果再返回主模型继续判断、检查或输出。

换句话说：

> **模型本身，也可以成为另一个模型的工具。**

这使得你可以构建自己的模型调度策略，而不是简单地把所有请求全部交给最昂贵的模型处理。

------

# 三种 AI 协议，一个 Gateway

1flowbase 当前支持主流 AI API 协议之间的转换：

- **Anthropic Messages**
- **OpenAI Chat Completions**
- **OpenAI Responses**

你的应用可以使用统一入口访问不同模型和工作流。

这意味着应用层不必因为模型供应商变化而不断调整自己的调用方式。

------

# AI Gateway 也是你的数据入口

1flowbase 会保存经过 Gateway 的完整 AI 会话数据，并存储到 PostgreSQL。

这意味着：

> **你不仅在分发 AI，也在持续积累属于自己的 AI 使用数据。**

对于企业 AI 团队，这些数据可以继续用于：

- AI 使用分析
- 用户行为分析
- Agent 行为分析
- 成本分析
- 模型质量分析
- Prompt / Harness 优化
- 路由策略优化
- 组合模型优化
- 业务系统建设

例如教育场景：

```text
学生
 │
 ▼
AI Gateway
 │
 ▼
AI Tutor / Agent
 │
 ▼
Models
```

学生与 AI 的交互可以通过 Gateway 沉淀下来。

然后继续构建：

```text
Conversation Data
        │
        ▼
学习行为分析
        │
        ▼
知识点与问题分析
        │
        ▼
学生学习报告
        │
        ▼
教师 Dashboard
```

于是 AI Gateway 从单纯的模型代理层，变成 AI 能力进入业务系统的统一入口。

------

# 用自己的 AI 数据优化自己的 AI

完整会话数据还可以形成一个持续优化闭环：

```text
AI Usage
   │
   ▼
Conversation Data
   │
   ▼
Analysis
   │
   ├── Improve Prompt
   ├── Improve Harness
   ├── Improve Routing
   └── Improve Model Composition
              │
              └───────────┐
                          ▼
                      AI Usage
```

例如你可以逐渐回答这些问题：

- 哪些请求最贵？
- 哪些任务根本不需要高能力模型？
- 哪些任务经常失败？
- 哪种模型组合性价比最高？
- Agent 在什么阶段最容易出现错误？
- 哪些 Prompt 或 Harness 效果最好？
- 哪些任务应该升级到更强模型进行审计？

因此，Gateway 不只是一个请求入口。

它还可以成为你的 **AI Feedback Loop**。

------

# 从 AI Gateway 继续构建应用

AI Gateway 是 1flowbase 的起点，而不是边界。

## AI Gateway

统一接入模型、转换协议，并对 AI 能力进行分发。

支持把普通模型和 Workflow 统一发布为 Gateway 中的模型。

------

## Workflow

通过 Workflow 组合：

- 模型
- 工具
- API
- 业务逻辑

然后把整个 Workflow 作为新的 AI 能力发布。

```text
Models + Tools + Logic
          │
          ▼
       Workflow
          │
          ▼
    Virtual Model
          │
          ▼
      AI Gateway
```

------

## API-first Backend

1flowbase 后端能力本身通过 API 提供。

GUI 能完成的运行时操作，本质上都建立在 API 之上。

这也是 Agent 可以进一步管理整个系统的基础。

------

## Data Source

当前 1flowbase 使用 **PostgreSQL** 作为主数据源。

除了保存 AI 会话之外，你还可以动态创建业务数据表，并通过系统能力对数据进行管理。

因此你的应用可以同时拥有：

```text
AI Data
+
Business Data
```

而不是把 AI Gateway 和业务数据库完全割裂。

------

# MCP Gateway

1flowbase 可以将 API 和 MCP Tool 统一暴露给 Agent。

但一个真实应用很容易拥有几十、几百甚至更多工具。

把所有 Tool 一次性塞进 Agent Context 并不是一个好的方案。

因此 1flowbase 使用渐进式工具发现：

```text
list
 │
 ▼
发现有哪些 Tool

get
 │
 ▼
获取 Tool 的完整定义

call
 │
 ▼
执行 Tool
```

Agent 只需要理解三个基础工具：

**list / get / call**

然后根据当前任务逐步发现和调用真正需要的能力。

这可以避免一次性把大量 Tool 定义塞进上下文。

------

# React Blocks

业务系统最终仍然需要 UI。

1flowbase 提供基于 **React** 的代码区块能力。

每一个 Block 都可以直接编写 React 代码，因此你不必被固定的低代码组件限制。

例如，Ant Design 官方示例代码可以直接作为一个 Block 的基础：

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

你可以继续使用整个 React 生态来构建自己的业务界面。

因此 1flowbase 的目标并不是提供一个封闭的拖拽式页面编辑器。

而是：

> **在低代码效率与原生 React 自由度之间取得平衡。**

------

# Built for Agents

我们正在进入一个新的软件阶段：

> **Build for Agents.**

但今天很多所谓“为 Agent 构建”的系统，实际上只是给 Agent 增加几个 Tool。

基础设施本身仍然主要是为人设计的：

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

1flowbase 的思路不同。

如果 GUI 可以完成一件运行时操作，那么这项能力也应该能够通过 API 暴露。

而 API 又可以进一步通过 MCP 暴露给 Agent。

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

因此，人和 Agent 操作的是同一个系统。

在 1flowbase 的运行时中，Agent 可以进一步参与：

- 创建和修改 Gateway
- 配置模型
- 创建 Workflow
- 创建数据表
- 管理业务数据
- 创建和调用 API
- 查询运行日志
- 配置权限
- 构建 React 界面

因此我们想解决的问题不是：

> **如何给应用加一个 Agent？**

而是：

> **如何构建一个真正为 Agent 而生的应用系统？**

------

# 从 AI Gateway 到完整应用系统

最终，你可以把这些能力组合起来：

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

也就是说：

> **在 AI Gateway 上构建一个真正为 Agent 而生的完整应用系统。**

------

# 一个 Agent 可以做到什么？

例如把 1flowbase MCP 接入 Codex：

```text
Codex
  │
  ▼
MCP
  │
  ▼
1flowbase
```

Agent 可以根据你的业务需求继续操作 1flowbase：

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

## 当前状态

1flowbase 的底层运行时已经采用 API-first 的方式构建，因此这些能力可以被进一步暴露给 MCP。

目前我们正在持续优化：

- 官方 MCP Tool 定义
- Tool 描述
- Agent Context
- 默认配置
- 应用模板

目标是让 Codex 等 Coding Agent **不需要阅读 1flowbase 源码，也能够理解平台能力并直接构建应用**。

当前阶段，在复杂应用构建场景下，仍建议让 Coding Agent 在 1flowbase 项目上下文中工作。

------

# 开箱即用的应用模板

1flowbase 本身拥有较多能力。

这也是我们目前正在重点改进的问题之一：

> **强大的系统，不应该要求每个新用户从空白画布开始。**

因此我们正在将常用能力封装成应用模板。

例如未来可以直接从：

```text
AI Gateway
Multi-model Router
Enterprise AI Gateway
AI Education Platform
Agent Application Backend
```

等场景开始。

你可以先使用模板完成 80% 的基础配置，再根据自己的业务需求继续扩展。

------

# 快速开始

## Linux / macOS

运行官方部署脚本：

```bash
curl -fsSL https://raw.githubusercontent.com/taichuy/1flowbase/main/scripts/shell/docker-deploy.sh | sh
```

按照提示完成配置后即可启动 1flowbase。

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

整个过程采用 Docker 部署，不需要手动搭建复杂运行环境。

------

# 适合谁？

## 企业 AI 团队

如果你的组织需要统一分发：

- OpenAI
- Anthropic
- 其他模型供应商
- 自定义模型
- 组合模型
- Agent Workflow

同时希望：

- 掌握自己的 AI 数据
- 分析企业 AI 使用行为
- 构建内部 AI 应用
- 为不同部门提供统一 AI 能力

1flowbase 可以作为这一层基础设施。

------

## AI 产品团队

如果你正在构建 AI SaaS 或 Agent 产品，可以从 Gateway 开始，继续扩展：

```text
Gateway → Data → API → Workflow → UI
```

而不必分别搭建多个互不关联的系统。

------

## 高级个人用户

如果你同时使用多个模型，也可以利用 Workflow 构建自己的组合模型：

> 高能力模型负责决策，低成本模型负责执行。

然后通过统一 API 将它当成一个模型使用。

------

# 1flowbase 与其他工具有什么不同？

| 项目       | AI Gateway | 组合模型 / Workflow | 完整会话数据 | 动态业务数据 | MCP      | React 应用   |
| ---------- | ---------- | ------------------- | ------------ | ------------ | -------- | ------------ |
| 1flowbase  | ✅          | ✅                   | ✅            | ✅            | ✅        | ✅            |
| LiteLLM    | ✅          | —                   | 部分         | —            | —        | —            |
| OpenRouter | ✅          | —                   | 平台托管     | —            | —        | —            |
| Dify       | 部分       | ✅                   | ✅            | —            | ✅        | 固定应用形态 |
| Supabase   | —          | —                   | —            | ✅            | 生态能力 | 前端自建     |
| n8n        | —          | ✅                   | —            | —            | 生态能力 | —            |

------

# 数据存储

1flowbase 当前使用 PostgreSQL 作为主数据源。

由于完整保存 AI 会话可能产生较大的数据量，对于高流量生产环境请提前评估：

- 存储容量
- 数据保留周期
- 数据备份策略
- 合规要求

我们也计划继续加强围绕 AI 数据的生命周期管理能力。

------

# Roadmap

当前重点包括：

-  更简单的默认 AI Gateway 配置
-  常用模型路由模板
-  组合模型模板
-  企业 AI Gateway 模板
-  优化 MCP Tool 描述
-  提升 Codex 等 Coding Agent 对系统能力的理解
-  降低 Agent 构建应用时对项目源码上下文的依赖
-  完善 AI 会话数据管理与生命周期策略
-  更多开箱即用的业务应用模板


--- 

# Star 1flowbase

如果你也认为未来的软件不只是：

> **Built with AI**

而应该进一步：

> **Built for Agents**

欢迎给 1flowbase 一个 ⭐。

我们想探索的是：

> **当 AI Gateway、业务数据、API、Workflow 和 UI 都能够被 Agent 操作之后，应用应该变成什么样？**

---

## 使用教程

- [让 GLM-5.2 在 Claude Code 里看图](https://github.com/taichuy/1flowbase/wiki/Make-GLM-5.2-See-Images-in-Claude-Code-with-1flowbase-CN)
- [Fusion 风格工作流：把多模型评审团发布成一个可观测的虚拟模型](https://github.com/taichuy/1flowbase/wiki/Fusion-Style-Workflow-CN)
- [1flowbase Wiki](https://github.com/taichuy/1flowbase/wiki)

---

## 仓库目录布局

```text
web/          前端根目录，基于 pnpm + Turbo 运作
api/          Rust 后端 Workspace 工作区
api/apps/     后端服务入口
api/crates/   共享后端 Crate 包
api/plugins/  插件源码工作区、HostExtension 清单与模板
docker/       本地中间件编排与自托管服务栈
scripts/      仓库级开发、测试、验证与调试脚本
```

---

## 参与贡献

非常欢迎社区贡献。在提交 Pull Request 之前，请运行以下验证脚本：

```bash
node scripts/node/verify.js repo
```

项目开发指导准则：

- [AGENTS.md](../../AGENTS.md)
- [web/AGENTS.md](../../web/AGENTS.md)
- [api/AGENTS.md](../../api/AGENTS.md)

---

## 友情链接

- [Linux.do](https://linux.do/) - 学 AI，上 L 站。
- [Aionui](https://github.com/iOfficeAI/AionUi) - 手机远程控制 AI 干活。
- [OfficeCLI](https://github.com/iOfficeAI/OfficeCLI) - 专为 AI 智能体设计的 Office 套件。
- [deepseek-pp](https://github.com/zhu1090093659/deepseek-pp) - DeepSeek 网页对话浏览器扩展插件。
- [MuseAI](https://github.com/yejiming/MuseAI) - 本地 AI 伴侣、文字冒险与穿书互动应用。
- [FrontAgent](https://github.com/FrontAgent/FrontAgent) - 专为前端工程设计的 AI Agent 系统。
- [RedBox](https://github.com/Jamailar/RedBox) - 面向小红书创作者的本地化 AI 创作工作台。

---

## 协议

本项目基于 [Apache-2.0](../../LICENSE) 开源协议授权。

---

## 贡献者

<p align="center">
  <a href="https://github.com/taichuy/1flowbase/graphs/contributors">
    <img src="https://contrib.rocks/image?repo=taichuy/1flowbase&max=50" alt="Contributors" />
  </a>
</p>

---

## Star 增长历史

<a href="https://www.star-history.com/?type=date&repos=taichuy%2F1flowbase">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=taichuy/1flowbase&type=date&theme=dark&legend=top-left&sealed_token=FqLzVSU8-9DxFglG-qgV59WwozJJfOHYwvjWNeVtnDP8OJ8r8BwvdLCIloKkdrLXWJqUEaD9xkVSr0RkCvzGaIxDYXYX2Zz53ikx7xZkZckNqgevZkOi1A" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=taichuy/1flowbase&type=date&legend=top-left&sealed_token=FqLzVSU8-9DxFglG-qgV59WwozJJfOHYwvjWNeVtnDP8OJ8r8BwvdLCIloKkdrLXWJqUEaD9xkVSr0RkCvzGaIxDYXYX2Zz53ikx7xZkZckNqgevZkOi1A" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=taichuy/1flowbase&type=date&legend=top-left&sealed_token=FqLzVSU8-9DxFglG-qgV59WwozJJfOHYwvjWNeVtnDP8OJ8r8BwvdLCIloKkdrLXWJqUEaD9xkVSr0RkCvzGaIxDYXYX2Zz53ikx7xZkZckNqgevZkOi1A" />
 </picture>
</a>

---

<div align="center">

**如果你希望 Agent 跨 AI、MCP、应用后端与 React 界面搭建并运营自托管应用，欢迎给 1flowbase 点一个 Star。**

[报告 Bug](https://github.com/taichuy/1flowbase/issues) · [提出新需求](https://github.com/taichuy/1flowbase/issues)

</div>
