# 1flowbase 仓库导航

| 要找什么 | 从哪里开始 |
| --- | --- |
| 后端入口、业务状态、存储与插件 | [api/AGENTS.md](api/AGENTS.md)；crate 归属与依赖见 [crate 地图](api/crates/AGENTS.md) |
| 前端路由、页面、组件与请求消费 | [web/AGENTS.md](web/AGENTS.md)；视觉和交互约定见 [DESIGN.md](DESIGN.md) |
| 跨模块设计依据 | [docs/architecture](docs/architecture/README.md) 按主题查；历史方案不代替当前实现 |
| 本地启动与运行态取证 | [scripts/README.md](scripts/README.md) 的 `dev-up` 是启动入口；受保护页面用 `page-debug`、认证 API 用 `api-debug`，见[临时会话约定](.agents/skills/frontend-development/references/browser-verification.md) |
| 镜像、Compose 与 CI | 部署配置从 [docker/README.md](docker/README.md) 找；自动化触发条件在 [.github/workflows](.github/workflows) |
| 项目工作方法 | [.agents/skills](.agents/skills) 是源目录，其他注册位置是镜像 |
| 本地协作偏好 | 遵循下方记忆入口；`.memory/user-memory.md` 私有，不提交 |

## 存放原则

- 文件跟随负责该行为的模块；进入目标子目录，先看最近的 `AGENTS.md`。测试默认随 owner 放在 `_tests/`，集成目标按当地约定。单目录接近 15 个文件或代码文件接近 1500 行时按职责拆分。
- 后端拥有业务状态与 DTO；前端保留接口字段原名，展示名放在 UI / i18n。临时字段兼容见[接口消费约定](.agents/skills/frontend-development/references/consumer-contracts.md)。
- warning、coverage 和长测试日志放 `tmp/test-governance/`。读取未知长度的输出先限字节（如 `COMMAND 2>&1 | head -c 4000`）；需保留退出码的构建或测试先写日志，再摘读。

## 按任务选 skill

方向未定（包括“为什么两个入口不一致”）用 `problem-framing`；目标明确且已授权实现，直接用 `frontend-development` / `backend-development`。开发中选验证方式查 `test-driven-development`；集中验收、发版或质量审计用 `qa-evaluation`。具体工程阻塞查 `github-solution-research`；只改 MCP 配置用 `mcp-configuration-development`。

目录规则说明「放哪里」（AGENTS 最多 200 行），skill 说明「怎么做」。同一条约定在负责它的地方维护，其他入口指向它。

@.memory/AGENTS.md
