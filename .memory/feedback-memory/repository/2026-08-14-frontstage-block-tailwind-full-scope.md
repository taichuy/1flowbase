---
memory_type: feedback
feedback_category: repository
topic: 前台区块 Tailwind 按需编译与样式隔离
summary: 用户确认区块代码按需读取和有界编译；Tailwind 是前端编译期能力，按有限候选集合生成内容寻址 CSS，以 ShadowRoot 隔离；后端仅持久化源码与依赖声明。
keywords:
  - frontstage
  - JS Block
  - Tailwind CSS
  - source-driven-utilities
  - ShadowRoot
  - call-by-need
  - content-addressed-css
match_when:
  - 设计或修改前台 JS Block 的 Tailwind 导入、编译、运行时样式与能力边界时
created_at: 2026-08-14 11
updated_at: 2026-08-14 21
last_verified_at: 2026-08-14 21
decision_policy: direct_reference
scope:
  - web/packages/tailwindcss-catalog
  - web/packages/page-runtime
  - web/app/src/features/frontstage
---

# 前台区块 Tailwind 按需编译边界

## 时间

`2026-08-14 21`

## 规则

区块代码采用 call-by-need：只有需要运行、预加载或编辑的区块才读取和编译；连续修改使用 latest-wins、singleflight、内容寻址缓存与有界 Worker 调度。

Tailwind 是前端 executable style 的编译期能力，不是需向每个区块分发全量 CSS 的运行时注册包。CSS identity 由 Tailwind compiler ABI 与排序去重后的有限候选集合共同决定，不绑定完整源码；普通 JavaScript 变化但 class 集合未变化时不重新编译 CSS。编译产物只存在于前端 L1 内存与 L2 IndexedDB 字节预算缓存中，并在 ShadowRoot 中隔离；相同 digest 共享 Constructable Stylesheet。

后端只持久化 `source_code`、用户依赖声明与读取定位所需的最小元数据，并通过 source revision 提供乐观并发保护；不拥有 Tailwind 编译、`generated_css`、compiler identity、编译状态或前端缓存语义。源码按需获取、有限候选分析、有界 Worker 编译、latest-wins、singleflight、派生产物与本地缓存均属于前端。没有明确的用户内容保留需求时，不为近期开发记录引入后端升级工具或兼容分支。

接受普通 Tailwind 构建语义：完整静态 class、有限条件和有限映射自动支持；无法从有限源码表达式推导的动态 class 使用 safelist 或明确诊断，不再承诺无边界字符串拼接自动可用。

## 原因

全量 preset 的 CSS 规模是 `O(N×(1+V))`，而按区块候选生成为 `O(K)`，通常 `K ≪ N×(1+V)`。ShadowRoot 已拥有选择器隔离所需的状态与控制能力；全量 preset 不能增强隔离，只会放大传输、解码、CSSOM 与多区块编译成本。

## 适用场景

前台 JS Block 的按需源码读取、有界编译调度、Tailwind compiler、前端派生 CSS、ShadowRoot 样式注入、依赖锁、source revision 和相关运行态测试。

## 备注

本条取代 `2026-08-14 11` 时“为避免静态候选缺失而提供全量 preset”的旧解释。JavaScript 包的版本锁、安全、权限和运行时确定性仍保留，不因 ShadowRoot 隔离而取消。
