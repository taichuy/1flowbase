## Scope
- 作用域：`web/` 及其子目录。
- 当前阶段：不要继续把混合职责堆进 `app/src/app/router.tsx` 和 `app/src/styles/globals.css`。

## Skills
- 做前端实现、页面、壳层、组件、交互时：使用 `frontend-development`
- 涉及导航、层级、入口、详情容器、L0 / L1 / L2 / L3 或同类对象行为时，用 `frontend-development` 的交互架构 gate 核对既定规则；未决产品取舍交给 `problem-framing`，已确认方向不重复审批
- 做质量评估、回归审计时：使用 `qa-evaluation`

## Directory Rules
- `app/src/` 是主前端源码根；下面提到的 `app/`、`app-shell/`、`routes/`、`features/`、`shared/`、`state/`、`styles/`、`style-boundary/` 默认都指 `app/src` 下对应目录。
- `app/` 只保留应用启动、Provider 组装、入口级装配。
- `app-shell/` 只承载共享壳层和壳层级菜单，不承载 route tree、feature 页面容器或 feature 私有组件。
- `routes/` 负责 `route id / path / selected state` 与权限消费 / guard 配置；接口授权真值属于后端。
- `features/*/pages` 放页面容器，`features/*/components` 放 feature 内部组件。
- `features/*/api` 放 feature 级请求消费层，例如 query key、queryFn、mutation 和当前 feature 的请求适配。
- `features/*/hooks` 放 feature 内部交互 hooks；跨 feature 复用前不要上提。
- `features/*/store` 放 feature 私有客户端状态；跨页面共享状态才进入 `state/`。
- `features/*/schema` 放 feature 私有 schema UI adapter、fragment、renderer registry。
- `features/*/lib` 放 feature 内部工具，不对其他 feature 默认开放。
- `shared/ui` 放跨 feature 复用组件，不承担 `app-shell` 专属结构；多 section 页面优先复用 `shared/ui/section-page-layout`。
- `shared/utils` 只放纯函数工具，不放请求、副作用和界面组件。
- `shared/api` 只放多个 feature 共同依赖的请求编排；若只是单 feature 使用，优先留在 `features/*/api`。
- `state/` 只放跨页面共享的客户端状态；feature 私有状态留在 feature 内，服务端数据优先留在 query/mutation 消费层，不要塞进 store。
- `styles/` 只放 token、reset、global 和应用级样式边界；feature 或页面专属样式应跟最近的页面、组件或壳层 owner 放在一起。
- `test/` 只放全局测试 setup 和跨场景测试工具；业务测试文件必须进入最近的 `_tests/`。
- `style-boundary/` 只负责样式场景注册和样式边界回归，不负责泛 UI 质量结论。
- `packages/api-client` 放底层原始请求 client、DTO、transport；页面和组件里不要直接写请求函数。
- `packages/ui` 只放设计系统级通用组件；单 feature 组件和 `app-shell` 专属结构不要上提到这里。
- 其他 `packages/*` 只在需要脱离 `app/` 独立复用、发布或承载 runtime/protocol/schema/SDK 时出现；不要为了“更整洁”把普通页面逻辑提成 package。
- 同一目录下文件数量接近 `15` 个时，先按职责继续收纳子目录，不要继续横向摊平；单文件接近 `1500` 行时先拆分职责。

## Local Rules
- 优先复用 `@1flowbase/ui` 与 `antd`，不要重复造轮子。
- 报表 / 图表能力默认以 `echarts` 作为宿主渲染依赖；不要新增 `echarts-for-react` 或其它图表 wrapper，除非先说明维护收益、安全影响和替代验证。
- 低代码 JS Block 图表只能通过受控 `Chart / EChart` primitive / facade 暴露；用户代码不得直接 import `echarts` 或任意 npm 包，图表 `option` 第一版必须是可校验 JSON，不开放 formatter 函数、custom series、HTML tooltip、外部图片或地图资源。
- UI 禁止出现内部提示词、调试文本、占位文案、mock 文案、待办/修复标记、异常对象、原始 JSON。
- 未开放功能不要写 `placeholder / reserved / later`；改为隐藏入口或正式“未开放/建设中”状态。
- 仅开发辅助信息允许在 `import.meta.env.DEV` 下渲染。
- 路由相关改动必须同步维护导航文案、`route id`、`path`、选中态和权限键。
- 前端消费后端字段时保持接口字段原名；表格列名、文案和 i18n label 可独立展示，但不要把同一字段映射成新的业务字段名。
- 菜单、route guard 和角色编辑消费后端权限 / catalog；`feature_id` 注册归属不等于整组接口授权，关闭权限 group 不清空自定义 operations。细则见 [consumer-contracts.md](../.agents/skills/frontend-development/references/consumer-contracts.md)。
- 前端连接、监听与请求状态不替代后端业务状态；断流、组件卸载或请求 abort 不证明业务取消、成功或回滚，显式取消与终态确认遵循后端 contract。
- 前端 `i18n/` key 没有静态代码引用时由 `i18n-hygiene` 以 `unused-i18n-key` warning 暴露；保留动态 key 必须写明原因。
- 样式改动固定按 `theme token -> first-party wrapper -> explicit slot -> stop`；禁止裸写 `.ant-*` 递归覆盖。
- 管理台/后台页面禁止 `Card` 套 `Card` 和卡片墙式堆叠；优先使用 `Table`、`Descriptions`、`Form`、`Typography`、`Divider`、`Space/Flex` 组织信息。
- 前端测试资源限制统一由仓库根 `.1flowbase.verify.local.json` 驱动；需要调整 `turbo` 并发或 `vitest` workers 时，同步更新 `.1flowbase.verify.local.json.example`，不要在 `web/package.json` 或 `web/app/package.json` 重新写死并发。
- 需要吃到本地资源限制时，优先走仓库标准入口：`pnpm --dir web test`、`pnpm --dir web/app test`、`node scripts/node/test-frontend.js fast|full`；不要直接用裸 `pnpm exec vitest` 或 `pnpm exec turbo` 绕过限制。
- 页面开发顺序先做“页面 + 组件组合”，再做“页面布局 + 组件样式调整”，除非用户同意，否则不对上层或者全局样式进行修改或者调整
- `packages/api-client/src` 当前是按 console resource 平铺的历史结构；新增 client 模块前先按平面或 feature 收纳，不继续扩大根层平铺。
- Model provider 页面用例放在 `features/settings/_tests/model-providers-page/` 场景文件内，不再回退到单个页面大测试文件。

## Verification
- 自检、验收、回归或交付使用 `qa-evaluation`，默认按 Dev Acceptance Gate 复用当前改动的有效证据；证据足够即停，重型全量门禁留给 beta / CI / 专门质量工作区。
- 导航、壳层、共享 / 全局样式或第三方 slot 覆写的验收需受影响场景的 `style-boundary` 证据；仅 DTO、请求逻辑或规则文档变化不因此触发样式门禁。
- 页面视觉 / 响应式结论需受影响页面的桌面与移动端证据；局部截图不证明全站 UI 通过，纯非视觉改动不默认重跑双端浏览器检查。
- 受保护页面取证使用 `node scripts/node/page-debug/cli.js`，认证 API 使用 `node scripts/node/tooling.js api-debug`；自定义流程复用 `scripts/node/page-debug/auth.js` 的临时 session owner 并在 `finally` 回收，不直接调用 sign-in。
