# Frontend Quality Gates

## Scope

前端评估命中以下任一条件时，必须追加本清单：

- 导航、路由、壳层、共享布局、共享菜单、主题或样式 token 发生变化
- 命中 `Ant Design`、`xyflow` 或其他第三方组件的样式覆写
- 用户明确提到“风格”、“UI 质量”、“看起来不对”、“样式被覆盖”
- 改动范围涉及前端目录结构、组件边界、导航信息架构

## Gate 1: UI Quality Is Acceptance

- 风格和 UI 质量本身就是验收项，不接受“功能可用但视觉先不管”
- 不能只给代码结论，至少要有截图、真实页面或可复现交互证据
- 若评估对象是单一路由或受保护页面，优先使用 `node scripts/node/page-debug.js snapshot|open ...` 产出运行态证据，而不是只靠人工口述
- 如果首屏层级、组件对齐、状态表达或浮层布局明显破坏，即使功能可点，也不能判通过

## Gate 2: Style Boundary

样式改动必须落在以下允许层级之一：

| 层级 | 是否允许 | 典型形式 |
| --- | --- | --- |
| `Theme Token` | 允许 | `ConfigProvider` token、主题变量、全局颜色/圆角/阴影 |
| `First-Party Wrapper` | 允许 | 自有 class 控制容器、留白、边界、布局 |
| `Explicit Slot Override` | 谨慎允许 | 从自有 wrapper 出发，命中单一明确 slot，做字体/颜色/圆角/外层间距校正 |
| `Recursive Internal Chain` | 禁止 | 裸 `.ant-*`、多级后代递归、把第三方内部 DOM 当自家结构长期维护 |

默认禁止项：

- 裸写 `.ant-*` 或其他第三方库全局类
- 跨多个第三方内部节点写后代链
- 无说明地修改第三方内部布局指标：
  - `display`
  - `position`
  - `height`
  - `min-height`
  - `line-height`
  - `padding`
  - `gap`
  - `overflow`

若命中上述内部布局指标，至少要补：

- 自有 wrapper 作为边界锚点
- blast radius 说明
- 其他消费者回归
- 真实运行证据

## Gate 3: Navigation Truth Layer

- 导航文案、`route id`、`path`、选中态规则和权限 key 必须来自同一配置真值层
- 允许业务友好文案映射到技术路径，但映射关系必须集中、显式、可测试
- 只改 label 不改路由映射、只改路径不改选中态逻辑、只改菜单不改权限键，都算不通过

## Gate 4: Directory And Boundary

- 目录结构应尽量对齐 spec 中的 `app-shell / routes / features / embedded / _tests`
- 测试文件必须进入 `_tests`
- 共享壳层、导航配置、路由注册、feature 页面不应长期堆在同一个 God file
- 文件和目录压力需要被显式检查：
  - 单文件不应无边界承载路由、导航、菜单、权限、状态与视觉逻辑
  - 单目录文件数过多时应收纳

## Gate 5: Runtime Style Regression Evidence

- `web/app/src/style-boundary/scenario-manifest.json` 只维护三件事：页面场景、组件场景、文件影响面映射
- manifest 中的 `propertyAssertions` 只用于样式边界断言，不用于泛 UI 质量主观判断
- 前端 QA 只知道页面路由、需要自动登录、稳定等待、截图、控制台或 `html/css/js` 证据时，优先运行 `node scripts/node/page-debug.js snapshot <route> --wait-for-selector ...`
- 页面存在规范化跳转时，补 `--wait-for-url <final-url>`；报告里应写明请求路由与最终 URL，避免把旧路由口径当成当前事实
- `page-debug` 成功后，报告应至少引用 `outputDir` 和其中的 `meta.json`、`page.png`、`console.ndjson`；若需要 DOM / 资源证据，再引用 `index.html`、`css/`、`js/`
- 导航、共享壳层、全局样式、第三方 slot 覆写改动后，必须至少运行一次 `node scripts/node/check-style-boundary.js component|page|file ...`
- `--file` 模式若提示“样式扩散失败”，视为文件影响面映射缺失，门禁未通过，先补场景映射
- 若输出“样式边界失败”，视为声明边界属性被打坏；报告必须包含场景 ID、关键节点、样式属性、实际值、命中的 selector，以及 `uploads/` 中的截图

## Default Severity Hints

| 场景 | 建议严重度 |
| --- | --- |
| 无边界第三方递归覆盖导致原生组件布局或交互被打坏 | `High` |
| 共享壳层样式变化未检查其他消费者 | `High` |
| 导航文案与路由真值层不一致，导致路径或选中态失真 | `Medium` |
| UI 质量缺少证据，只以“代码看起来合理”判断通过 | `Medium` |
| 目录边界失焦、文件职责发散，但暂未直接造成功能错误 | `Medium` |

## Evidence Checklist

- `page-debug` 命令、请求路由、最终 URL、`outputDir`
- `page.png`、`console.ndjson`、`meta.json`，必要时补 `index.html` 与资源快照
- 关键页面截图或真实运行结果
- 相关 CSS 选择器或主题 token 证据
- 路由配置与导航配置对照
- 受影响消费者回归结果
- 目录树 / 文件职责 / `_tests` 对照
