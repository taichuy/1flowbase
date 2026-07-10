# Project Evaluation Checklist

## Context First

- 阅读 `.memory/AGENTS.md`
- 阅读 `.memory/user-memory.md`
- 阅读项目记忆和反馈记忆中与当前项目相关的内容
- 阅读与评估范围直接相关的 spec、模块 README、设计稿和近期 QA 记录
- 如果评估范围命中后端，优先对齐最近的后端项目记忆，例如接口内核、插件边界、质量规范和实施计划阶段记忆

如果项目记忆或反馈记忆为空，应在报告里明确说明上下文缺口。

## Coverage Matrix

| 维度 | 要检查什么 | 典型证据 |
| --- | --- | --- |
| UI 一致性 | 壳层、画布、详情层、状态表达是否仍属于同一产品系统 | 截图、页面行为、视觉对照 |
| UI 质量门禁 | 风格和 UI 质量是否被当成正式验收项，而不是“能跑即可” | 截图、设计对照、关键页面真实体验 |
| 路由级运行态证据 | 单一路由、受保护页面、登录跳转和 ready signal 是否已被真实验证，是否记录请求路由与最终 URL | `page-debug` JSON、`outputDir`、`meta.json`、`page.png`、`console.ndjson`、最终 URL |
| 样式边界 | 是否存在无边界第三方样式递归覆盖、是否破坏原生组件布局或交互 | CSS 规则、wrapper class、blast radius 审查、组件回归 |
| 导航真值层 | 导航文案、`route id`、`path`、选中态和权限映射是否仍单一真相层维护 | 路由配置、导航配置、选中态逻辑、spec 对照 |
| 页面与流程逻辑 | 页面边界、入口设计、主路径、下钻模型是否成立 | 手动流程、结构对照、页面行为 |
| 响应式与降级 | 小屏策略是否诚实，是否按优先级重排而不是硬堆桌面结构 | 小屏截图、响应式断点验证 |
| API 契约 | 请求输入是否清楚、短、平、单动作，调用方是否仍成立；前端字段名是否沿用后端 DTO / 领域语义，是否存在未标记字段别名 | 接口定义、调用样例、OpenAPI / DTO、api-client 类型、日志 |
| 状态与数据一致性 | 状态集合、流转、展示和存储是否一致 | 状态字段、页面结果、日志、数据库样例 |
| 架构边界 | 核心规则、适配层、状态入口、插件边界是否被污染 | 代码结构、写路径、接口边界 |
| 后端三平面 | `public / control / runtime` 是否仍分离，接口和资源是否按正确平面归属 | 路由结构、service 调用链、OpenAPI |
| Resource Action Kernel / Dynamic Modeling | `Resource Action Kernel` 是否仍由宿主托管，`dynamic modeling` 是否仍是元数据系统而不是 runtime 数据本身 | resource/action registry、hook pipeline、descriptor、模型发布流程、runtime engine |
| 插件消费分类 | `HostExtension / RuntimeExtension / CapabilityPlugin` 是否仍按各自注册权、绑定方式和消费方式工作 | plugin-framework、runtime-core、host contribution、分配/绑定逻辑、provider/node/datasource/publish 配置 |
| HostExtension 启动面 | HostExtension manifest contribution、load plan、pre-state infra provider、route/worker/migration namespace 是否仍受宿主管理 | host-extension.yaml、loader、host infrastructure registry、route/worker/migration registry、PostgreSQL extension migration tracking |
| 工程质量门禁 | `route / service / repository / domain / mapper` 是否仍分层，`storage-durable/postgres` 内的 `storage-postgres` 是否保持 repository / mapper 拆分，`storage-durable / storage-object` 是否仍守住各自边界，验证命令和测试目录规则是否被执行 | 代码结构、storage-durable/postgres、storage-durable、storage-object 目录、测试文件、验证脚本、命令输出 |
| 测试缺口 | 当前项目最关键的行为是否缺少自动化或手动验证覆盖 | 测试文件、命令结果、缺口清单 |
| 目录与文件边界 | 目录规划是否仍对齐 spec，路由、壳层、feature、测试文件是否分工清楚 | 目录树、文件职责、spec 对照、行数与目录压力 |
| Maintainability / Dead Abstraction | 是否存在过度抽象、无用代码、空转封装、过期兼容层或无真实消费者的公共入口 | 调用图、导出点、运行路径、边界职责、废弃计划、`repo-hygiene`、churn |
| 热点预防层 | 近期反复修改是否暴露 AI 前置判断缺口，是否需要更新 skill、AGENTS、质量脚本或代码环境规则 | `git log`、`git numstat`、高频文件、现有 skill 缺口、建议 patch 点 |

## Output Rules

- 默认只出评估和报告，不直接进入修复
- Findings 按严重度从高到低排序
- 不要把“代码看起来合理”当成通过证据
- 若前端结论来自 `page-debug`，报告里应明确写出请求路由、最终 URL 和 `outputDir`
- 不能验证的项要单独列入 `未覆盖项 / 风险`
- 发现旧字段兼容 alias 时，必须列出 `@field-contract-compat` 标记、废弃计划、测试证据和 `repo-hygiene` warning；没有标记时按契约漂移报告
- 发现 maintainability / dead abstraction 问题时，必须列出证据和 warning；修复、删除、合并抽象或改公共接口前，需要用户明确同意
- 命中后端评估时，如果没有核对最新 `.memory/project-memory`，应在报告中明确说明结论可能存在旧口径偏差
- 命中热点 churn 复盘时，必须输出“AI 下次如何避免”的预防层建议；不能只给业务代码修复建议

## Escalation

- 前端结构问题联动 `frontend-logic-design`
- 前端实现一致性问题联动 `frontend-development`
- 后端契约、状态和边界问题联动 `backend-development`
