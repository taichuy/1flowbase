# 质量证据与规则所有权

质量门禁保护现有能力、职责边界和数据契约。功能完整性优先于可靠性、性能与可演进性；格式、文件长度和目录压力是调查入口，不能代替调用链与行为证据。

## 证据分层

- `verify.yml` 是 PR / push 合并基线；`quality-gate.yml scope=ci` 是完整项目体检，额外包含完整测试分片、Clippy、一致性、覆盖率和 React Doctor。
- 一次完整体检固定同一个 full SHA。根 `quality-gate-report.json` 必须为 `passed/0`，必需 component 唯一、成功且绑定同 SHA；四个 API coverage 分片经 merge 后才形成覆盖率 component。
- SDK / Host / EventBus 的单元测试由 runtime-storage-fast 执行；PostgreSQL adapter 与跨层 service 测试由 storage-postgres 分片执行；API lib 与 integration targets 由 API nextest 分片执行。临时专项 workflow 的存在不代表必须重复执行同一组行为。
- 定向 Cargo 命令必须有非零实际通过用例；编译、ignored 和空 filter 不等于行为通过。真实 managed worker 由 CI 先构建，再向行为测试注入路径。
- `state-protocols --skip-live-acp` 不验证真实外部 ACP 服务；静态边界、单元测试与 component 收据也不证明所有页面视觉或生产运行态已经验收。报告分别列明证据范围。

## 静态规则的边界

`check-rust-backend` 管理第一方 Rust 代码，排除 `api/vendor/` 的上游源码和安装产物。第三方漏洞与容器环境警告单列归因；不能借此排除第一方调用路径，也不能用路径包含 vendor 字样代替明确目录边界。测试中的合法失败注入与生产 panic 分别判断。

`schema-hygiene` 从正式 migrations 构造结构 inventory。合法表或 constraint 重命名保留原列、键与引用语义；constraint 改名后的删除按新身份处理。无法识别的 DDL 仍报告解析失败，不能把整条 migration 豁免。静态 inventory 不替代 PostgreSQL 的真实迁移执行、约束校验和事务验证。

规则自身需要合法反例与违规反例。删除旧断言的依据是当前 contract 和有效替代证据，不能只因为它失败，也不能反过来让产品兼容过期测试。文件压力、兼容标记与动态 i18n key 等第一方 warning 保留 owner 和保留原因，不以清空 warning 换取健康结论。

## 写入与读取职责

Trace projection 的写入对象由 `control-plane-contracts` 管理。共享 alias 使用 typed object 和 `Arc<Value>`，成员变更按 copy-on-write 隔离；标量和数组可作为普通 JSON 保留，但向其插入对象字段必须显式返回错误并保持原值。业务端传播错误，持久化边界才展开共享 alias，读取 DTO 与 wire JSON 形状保持不变。

选择性备份中的 PostgreSQL bytea 由存储 adapter 解码，使用既有 `base16ct`，保留大小写十六进制与空字节串并拒绝无效内容；业务层不重复实现编码算法。

## 执行位置

项目体检优先在 GitHub Actions 执行，持续失败的剩余门禁不超过三项时才考虑本地取证。warning、coverage 与执行日志统一放在 `tmp/test-governance/`。完整通过只结算已有证据覆盖的目标，不能据此宣称完成独立安全审计或全产品人工验收。
