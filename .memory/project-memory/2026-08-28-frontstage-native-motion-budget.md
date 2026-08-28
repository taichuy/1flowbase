---
memory_type: project
topic: Frontstage Native Block Runtime 统一动效预算
summary: #1927 已按最终三层优先级实现 Native Runtime Motion Profile：默认 direct，作者可显式 opt-in responsive，prefers-reduced-motion 始终 direct；定向 QA 已通过，等待用户验收。
keywords:
  - Frontstage
  - Native Block
  - Ant Design
  - motion token
  - prefers-reduced-motion
  - performance
created_at: 2026-08-28 21
updated_at: 2026-08-28 22
last_verified_at: 2026-08-28 22
decision_policy: verify_before_decision
status: user-acceptance
scope:
  - issue:1927
  - web/app/src/features/frontstage/lib/native-modules/native-motion-runtime.ts
  - web/app/src/features/frontstage/lib/native-trusted-block-react-adapter.tsx
---

# 当前决策

- 用户批准 Single Issue [#1927](https://github.com/taichuy/1flowbase/issues/1927)：由既有 Native Runtime Adapter 的 `ConfigProvider` owner 提供统一动效预算，不修改用户 Block、AntD/rc-menu/RGL 上游或 PageCanvas 尺寸算法。
- Runtime 默认注入 `motion:false` 与 fast/middle/slow `0s`，并关闭 AntD Wave；作者显式 `motion:true` 与 duration token 时 opt-in responsive，并恢复 Wave。
- 合并优先级固定为 `prefers-reduced-motion direct > 作者显式 responsive > Runtime 默认 direct`；reduced-motion 强制覆盖作者配置，但动态切换不 remount Block。
- 所有 Native Block 通过共享 `MediaQueryList + Set<listener>` 累积复用一个浏览器媒体查询监听；最后一个订阅者卸载后释放。

# 动机与边界

两个目标 Menu Block 没有请求或重型计算，交互期间目标 Block 没有 PageCanvas intrinsic height commit；无竞争环境下 React 首次反馈很快，主要感知延迟来自 AntD 内部 200～330ms motion。因此新增的是 Runtime motion governance，不继续堆尺寸缓存、网格索引或 Menu 特例。

# 验收候选

- Adapter TDD：默认 direct、作者 responsive opt-in、媒体偏好强制 direct 且动态切换不 remount。
- 4 个定向测试文件共 35 项通过；TypeScript 与 style-boundary `page.frontstage` 通过；ESLint 0 error，保留 Adapter 既有 fast-refresh warning。
- 三个真实 Menu Block 的目标 submenu 首次视觉反馈 P95 24.2ms、Event Timing P95 72ms、processing delay P95 19.9ms、motion cleanup P95 53.9ms、目标宿主高度提交 0、`motion-collapse / wave-motion` 变更 0、页面错误 0。
- `inlineCollapsed` 单独 P95 为 88ms；它会同步重构整棵用户 Menu DOM，不属于 AC-004 的叶子/一级/二级 submenu 统计，保留为源码级残余成本。4 个 Long Task 覆盖 15.4% 的 700ms 观测窗，原始证据保留但不冒充 AC 通过依据。
- reduced-motion 几何采样每次最多 2 个状态；该采样会主动增加 layout 成本，只用于行为证据，不作为性能结论。
- 截止日期未指定；用户真实页面验收后关闭 #1927。

# 最新验收反馈

- 用户在 Block `01a047b3-15d8-7192-a400-e2fb2859699c` 及相邻 `01a047b3-4a81-79e0-8e71-b8a8b6189f49` 发现叶子选择明显流畅，但 inline submenu 展开仍慢一拍。
- 真实浏览器证据：叶子选择约 10～28ms 完成且不创建 DOM；一级 submenu 约 98～121ms，二级约 95～133ms，相邻 Block 约 110～134ms。首次展开创建 10 个元素，后续展开不再创建，但仍保持约 100ms，因此 lazy mount 只解释首次少量开销，不是持续差异主因。
- 目标 Block intrinsic height commit 均为 0；差异来自 rc-menu 结构状态机的两帧 motion 启动、50ms token 与清理阶段，不是 PageCanvas、RenderIdentity 或尺寸缓存回归。
- 用户最终确认三层优先级并已完成实现：Runtime 默认 direct；作者显式 opt-in responsive；prefers-reduced-motion 始终 direct。
