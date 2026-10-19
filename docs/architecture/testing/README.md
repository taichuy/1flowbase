# 质量证据与规则所有权

质量门禁保护现有能力、职责边界和数据契约。功能完整性优先于可靠性、性能与可演进性；格式、文件长度和目录压力是调查入口，不能代替调用链与行为证据。

## 证据分层

- `verify.yml` 是 PR / push 合并基线；`quality-gate.yml scope=ci` 是完整项目体检，额外包含完整测试分片、Clippy、一致性、覆盖率和 React Doctor。
- 一次完整体检固定同一个 full SHA。根 `quality-gate-report.json` 必须为 `passed/0`，必需 component 唯一、成功且绑定同 SHA；四个 API coverage 分片经 merge 后才形成覆盖率 component。
- SDK / Host / 内存 EventBus 的单元测试由 runtime-storage-fast 执行，贡献契约与插件事件注册测试由 core-libs 执行；PostgreSQL adapter 与跨层 service 测试由 storage-postgres 分片执行；API lib 与 integration targets 由 API nextest 分片执行。临时专项 workflow 的存在不代表必须重复执行同一组行为。
- 定向 Cargo 命令必须有非零实际通过用例；编译、ignored 和空 filter 不等于行为通过。真实 managed worker 由 CI 先构建，再向行为测试注入路径。
- `state-protocols --skip-live-acp` 不验证真实外部 ACP 服务；静态边界、单元测试与 component 收据也不证明所有页面视觉或生产运行态已经验收。报告分别列明证据范围。

## 静态规则的边界

`check-rust-backend` 管理第一方 Rust 代码，排除 `api/vendor/` 的上游源码和安装产物。第三方漏洞与容器环境警告单列归因；不能借此排除第一方调用路径，也不能用路径包含 vendor 字样代替明确目录边界。测试中的合法失败注入与生产 panic 分别判断。

`schema-hygiene` 从正式 migrations 构造结构 inventory。合法表或 constraint 重命名保留原列、键与引用语义；constraint 改名后的删除按新身份处理。无法识别的 DDL 仍报告解析失败，不能把整条 migration 豁免。静态 inventory 不替代 PostgreSQL 的真实迁移执行、约束校验和事务验证。

规则自身需要合法反例与违规反例。删除旧断言的依据是当前 contract 和有效替代证据，不能只因为它失败，也不能反过来让产品兼容过期测试。文件压力、兼容标记与动态 i18n key 等第一方 warning 保留 owner 和保留原因，不以清空 warning 换取健康结论。

## 第一方维护告警

告警先按实际源码所有权归因。`repo-hygiene` 的 inventory 也可能包含 `api/vendor/`；这些条目是上游证据，不能把报告总数直接称为第一方问题数。`warningFiles` 是日志文件数，不是 finding 数。第一方 Hook 闭包、重复 React key、废弃组件参数与无效夹具属于可修问题，不能用“上游依赖告警”或“既有债”代替修复。

Hook 依赖调整保留 locale、actor、workspace 与 policy 的实际失效语义；不能为了 lint 删除隔离键，也不能加入不稳定的整个 query/session 对象导致重复连接。React Refresh 的组件边界与纯逻辑分别维护，不以虚假导出或规则排除绕过。运行夹具使用真实 Router context、唯一资源身份和正确的 ShadowRoot；不通过过滤 console 证明告警消失。

组件模块只承担渲染与资源装配。ECharts 的 `lifecycle.tsx` 保留 Boundary 入口，`lifecycle-context.ts` 管理资源 scope；窗口的 Provider / Window 与 controller、overlay context 分开，避免共享 Hook 变更被组件热更新边界吞掉。壳层 assistant 的工具执行与上下文订阅分别归 `assistant-client-tools/factory.ts` 和 `context.ts`，Provider 只组装当前 workspace、locale 和查询缓存。

编辑器的映射校验、排序算法与工具 DTO 草稿留在纯模块；schema field control renderer 保留返回实际输入控件的语义，让 Form.Item 属性注入和通知字段 cloneElement 继续成立。style-boundary 的场景组件与 HTTP 数据夹具分开，scenario manifest 同时追踪两个 owner；行为测试中的夹具提取不改变产品加载、mock 注册或断言的职责。

根前端 `lint` 入口向所有包的 ESLint 透传 `--max-warnings 0`，防止已清理的代码告警再次静默通过；hygiene 的动态引用、兼容期限和规模提醒仍按各自规则与证据结算，不因此升级或隐藏。

以下 i18n 告警有具体消费者或不同语义，继续由 `i18n-hygiene` 暴露；这不是扫描器豁免，也不表示所有未引用 key 都有效：

| Owner / key | 保留依据 |
| --- | --- |
| frontstage `auto.interfaces / auto.block_tree / auto.configuration` | `shared/code-block/BlockStudioWorkspace.tsx` 的 section `labelKey` 配置由同组件 renderer 翻译，属于配置驱动引用 |
| settings `auto.network_center_health_{healthy,not_tested,unhealthy,invalid}` | `settings/network-center/pools/NetworkEgressPoolsPanel.tsx` 从 health 值动态组成 key，筛选与状态标签共用相同语义 |
| app-shell `auto.subsystem` | `Navigation.tsx` 消费后端 `label_key`；导航测试和 style-boundary 场景包含该配置入口。当前内置导航不发布 embedded-apps，不能据此声称有现行生产菜单；保留为该配置翻译入口的资源 |
| frontstage `auto.operation / files.actions` | 通用操作与文件表操作拥有不同消费者；中英文翻译向量不同 |
| frontstage `auto.save / files.save` | 设计器保存与文件编辑器保存属于不同上下文 |
| frontstage `design.tab_route_segment / files.url` | 页签路由片段与后端文件 URL 是不同对象，不因中文同字合并 |
| notifications `recipient_count / system_notification_recipients` | 接收人数汇总与接收对象列表是不同语义 |
| ui-management `default / set_default` | 默认状态与设置默认的命令分别拥有消费者 |

字段兼容告警逐项核对 source、alias、remove_by 和真实读写测试；期限未到不自动说明可以删除，也不等于永久保留。协议负例中的 legacy/obsolete 文字承担说明被拒行为的职责，不通过删除注释消除告警。文件和目录压力提供拆分线索，改动触及的超大模块按真实职责收纳；未实施的全仓结构项在任务 inventory 中明确列出，不能把一轮修复或机械绿灯写成全仓结构债清零。

## 写入与读取职责

Trace projection 的写入对象由 `control-plane-contracts` 管理。共享 alias 使用 typed object 和 `Arc<Value>`，成员变更按 copy-on-write 隔离；标量和数组可作为普通 JSON 保留，但向其插入对象字段必须显式返回错误并保持原值。业务端传播错误，持久化边界才展开共享 alias，读取 DTO 与 wire JSON 形状保持不变。

选择性备份中的 PostgreSQL bytea 由存储 adapter 解码，使用既有 `base16ct`，保留大小写十六进制与空字节串并拒绝无效内容；业务层不重复实现编码算法。

## 执行位置

项目体检优先在 GitHub Actions 执行，持续失败的剩余门禁不超过三项时才考虑本地取证。warning、coverage 与执行日志统一放在 `tmp/test-governance/`。完整通过只结算已有证据覆盖的目标，不能据此宣称完成独立安全审计或全产品人工验收。
