---
memory_type: project
topic: Frontstage Native Block Runtime 统一动效预算
summary: 用户批准 #1927 在 Native Runtime Adapter 统一注入 AntD motion token；实现已通过 TDD、真实 Menu 性能校准与 reduced-motion 几何验证，等待用户验收。
keywords:
  - Frontstage
  - Native Block
  - Ant Design
  - motion token
  - prefers-reduced-motion
  - performance
created_at: 2026-08-28 21
updated_at: 2026-08-28 21
last_verified_at: 2026-08-28 21
decision_policy: verify_before_decision
status: user-acceptance
scope:
  - issue:1927
  - web/app/src/features/frontstage/lib/native-modules/native-motion-runtime.ts
  - web/app/src/features/frontstage/lib/native-trusted-block-react-adapter.tsx
---

# 当前决策

- 用户批准 Single Issue [#1927](https://github.com/taichuy/1flowbase/issues/1927)：由既有 Native Runtime Adapter 的 `ConfigProvider` owner 提供统一动效预算，不修改用户 Block、AntD/rc-menu/RGL 上游或 PageCanvas 尺寸算法。
- responsive profile 的 token 经 20 次真实页面样本校准为 fast 30ms、middle 50ms、slow 80ms；目标是吸收 rc-motion 固定流水线开销后，让用户可见完成 P95 不超过 160ms。
- 用户显式 theme token 在 responsive profile 中优先；`prefers-reduced-motion: reduce` 是可访问性约束，最终强制 `motion:false` 且三个 duration 为 `0s`。
- 所有 Native Block 通过共享 `MediaQueryList + Set<listener>` 累积复用一个浏览器媒体查询监听；最后一个订阅者卸载后释放。

# 动机与边界

两个目标 Menu Block 没有请求或重型计算，交互期间目标 Block 没有 PageCanvas intrinsic height commit；无竞争环境下 React 首次反馈很快，主要感知延迟来自 AntD 内部 200～330ms motion。因此新增的是 Runtime motion governance，不继续堆尺寸缓存、网格索引或 Menu 特例。

# 验收候选

- Adapter TDD：默认预算、用户 token 覆盖、媒体偏好动态切换且不 remount。
- 4 个定向测试文件共 35 项通过；TypeScript 与 style-boundary `page.frontstage` 通过；ESLint 0 error，保留 Adapter 既有 fast-refresh warning。
- 两个真实 Block 20 次点击：首次视觉反馈 P95 37.1ms、Event Timing P95 72ms、processing delay P95 11.7ms、motion cleanup P95 151.3ms、目标宿主高度提交 0、页面错误 0；1 个不可归因 Long Task 覆盖 5% 交互，保留在原始证据中且未形成系统性阻塞。
- reduced-motion 几何采样每次最多 2 个状态；该采样会主动增加 layout 成本，只用于行为证据，不作为性能结论。
- 截止日期未指定；用户真实页面验收后关闭 #1927。
