# Frontend Review Checklist

## Scope

实现期检查已确认结果与直接风险；进入自检 / 交付时由 `qa-evaluation` 选择 [gate lane](../../qa-evaluation/references/governance/gate-lanes.md)。本清单不要求运行所有检查，也不替代需求决策或验收点结算。

## Implementation Review

- 页面是否仍符合 `DESIGN.md` 的任务域、recipe 与 L1 模型？交互检查复用已确认主路径、同类对象行为和反馈位置，不重复需求模板。
- 请求、DTO、业务状态、客户端草稿与连接状态的 owner 是否清楚？权限和流式变更对照 [consumer-contracts.md](consumer-contracts.md)，不要用前端推断补后端真值。
- 是否把单 feature 编排强行提到共享层，或为拆分而增加一次性 helper / props 转发层？保留业务路径连贯，不用文件行数代替职责判断。
- 样式是否落在 token / 自有 wrapper / 显式 slot？状态色、第三方原生交互与受影响消费者是否仍符合现有规则？
- route id、path、选中态及权限消费是否一致？i18n 变更是否保留已有文案值、owner 与字段语义？
- 失败测试先区分产品回归、contract 破坏、环境问题与过期断言；不为兼容旧测试加 alias、fallback 或削弱新 contract。

## Evidence By Risk

| 本次变化 | 最小证据方向 | 不由该证据推导 |
| --- | --- | --- |
| 局部文案、间距、纯视觉调整 | 先给受影响区域可见效果，按确认后的 diff 选择必要检查 | 全量 build / lint / 全站 UI 已通过 |
| 页面布局或响应式 | 受影响页面的桌面与移动端关键场景，确认主任务、浮层与小屏降级 | 所有页面都正确 |
| 导航、壳层、共享样式或第三方 slot | 匹配影响面的 `check-style-boundary` component / page / file 证据；必要时页面截图 | 样式边界通过等于泛 UI 质量通过 |
| DTO、权限、状态与流式行为 | 复用定向 consumer test / TDD；类型风险补 tsc，真实环境存疑再补运行态取证 | mock 或 UI 隐藏证明服务端鉴权通过 |
| i18n 资源或 key 引用 | 按 [i18n-rules.md](i18n-rules.md) 做 owner 内核对并引用适用 hygiene 结果 | 为消除 warning 改展示文案 |
| 仅 AGENTS / skill 文档 | 引用与格式校验、历史冲突核对、[场景反例](../examples/pressure-scenarios.md) | 产品运行态已验收 |

命中样式影响时，使用 `node scripts/node/tooling.js check-style-boundary component|page|file ...`。共享样式 / slot 的场景与 `impactFiles` 在 `web/app/src/style-boundary/scenario-manifest.json` 维护；`boundaryNodes / propertyAssertions` 只描述边界属性，失败需保留场景、selector、实际值与截图证据。

浏览器取证按 [browser-verification.md](browser-verification.md)，检查 `meta.json / page.png / console.ndjson` 与业务 ready signal；缺少运行态证据时限制视觉结论。warning 与 coverage 产物落到 `tmp/test-governance/`。

## Completion And Stop

- 复用适用于当前改动的证据，只在实现、fixture、预期或相关环境改变，或仍有未覆盖风险时补跑。
- 当前结果与直接风险已覆盖即停止；完整 lint / build / style-boundary / i18n hygiene / verify-repo 默认交给 beta / CI / 专门质量工作区，成本与升级边界遵循 gate lane。
- 交付说明标明已验证、未验证和残余风险；有验收点编号时逐点映射，证据不足不写通过。
- 目标、权限、业务状态、contract 或验收语义变化才回到 `problem-framing`；既定范围内的局部实现选择不重新审批。
