---
memory_type: project
topic: IQLater 线上 Docker 登录入口与前端首屏分包热更状态
summary: 用户于 2026-11-22 批准并完成前端无痕冷启动、动态区块按需加载及登录后首屏商图凝聚与 K=1 SPT 预热数学化优化，已热更至 iqlater-ali 前端容器并完成实测验证。
keywords:
  - iqlater-ali
  - sign-in
  - cold-start
  - post-login
  - scenario-chunk-planner
  - authenticated-prewarm
  - native-modules
  - nginx
match_when:
  - 排查或调整 ai.iqlater.com 前端首屏打开速度、登录页冷启动、登录后工作台首屏加载、动态区块/动态表单分包或 Web 容器静态资源发布
created_at: 2026-11-20 20
updated_at: "2026-11-23 17"
last_verified_at: "2026-11-23 17"
decision_policy: verify_before_decision
scope:
  - docker/web/nginx.conf
  - web/app/build/
  - web/app/src/features/auth/
  - web/app/src/features/frontstage/lib/native-modules/
  - web/packages/page-runtime/src/native-react-compiler/
status: deployed
---

用户于 2026-11-20 在当前聊天要求修复线上登录入口未使用已安装 Docker 镜像产物的问题。动机是让实际入口跟随镜像版本；此次不是对编辑器懒加载、语言资源拆分或外层缓存改造的授权。无另定截止日期。

Root 在该日完成持久化 Web 配置与运行配置的窄修改：`/home/deploy/git/1flowase/docker/web/releases/85ee08b2/live.conf` 的 `/sign-in` root 指向 `/usr/share/nginx/html`，旧目录只保留 hash 静态资源回退；仅热加载 Web Nginx，没有重启容器或触碰 7600。

后续操作前重新核对当前 mounts、运行配置和公网文档，不把本记录当成实时部署事实。证据入口为本地 `tmp/test-governance/iqlater-docker-entry-fix-20261120/report.md`。服务器备份/回滚入口为 `/home/deploy/git/1flowase/docker/web/releases/docker-image-entry-20261120/`。其余登录后性能问题仍为未实施方案，后续需明确范围。

## 权限缓存排障边界（2026-11-20 22）

用户要求本地复现和修复动态路由勾选回退，留待下一版本发布；本轮没有修改线上角色、数据库、代理或部署。只读查询观察到权限 GET 的 `X-Cache: EXPIRED → HIT` 且缺少 Cache-Control；本地受控缓存复现了全选落库8项却回显旧5项、后续保存覆盖之前选择。下一次线上操作必须重验响应头与外层缓存配置，不能假设代码更新会清除已有缓存。证据入口：`tmp/test-governance/role-route-permissions-20261120/report.md`。系统 root 豁免语义保持不变。用户未指定部署截止日期。

## 前端无痕冷启动与动态区块按需分包热更（2026-11-22 14）

用户于 2026-11-22 确认执行平衡方案并授权热替换 `iqlater-ali` 前端容器静态产物进行线上实测：
1. `/` 与 `/sign-in` 对称启用无凭证 `sign-in-preload.html` + 文档级会话/登录入口并发探测，并由 `.gitattributes` 保证 `password_local.tsx` 跨平台 LF SHA-256 一致命中编译期快速通道。
2. 消除 `page-tree-icon-assets`、`native-dayjs`、`native-dnd-kit`、`native-antd-es` 重复虚拟模块叶子，使生产 JS 产物由 3,428 降至 2,065 个；`NativeReactModuleRegistry` 支持按 `importBindings` 精确加载 `antd`、`@ant-design/icons`、`@1flowbase/data-blocks` 具名导出并并行预热组件域；`antd` 启用拓扑优先级并隔离 `dayjs-runtime`/`query-runtime`/`router-runtime`/`i18n-runtime`。
3. 线上 `1flowbase-web-1` 容器已补齐 `.gz` 预压缩副文件并重载 `/etc/nginx/conf.d/default.conf` 与持久化 `/home/deploy/git/1flowase/docker/web/releases/85ee08b2/live.conf`；注意 `/usr/share/nginx/html` 位于容器可写层，若后续在推送新镜像前重建容器会回退至旧镜像静态层。
4. 备份与一键回滚入口：`/home/deploy/git/1flowase/docker/web/releases/perf-20261122/rollback.sh`；本地证据入口：`tmp/test-governance/bundle-perf-20261122/report.md`。

## 登录后首屏数学化分包凝聚与 K=1 SPT 预热热更（2026-11-22 17）

用户反馈无痕登录后首次进入工作台仍慢，并在“不动阿里云外层 Nginx、可改容器内 Nginx 与前端”的边界下确认执行数学优先平衡方案：
1. 最小割与拓扑优先级调整：懒加载 `McpAuthorizePage`、新增 `antd-locale-data` 切断 `antd-picker`、切换前台宿主至 `@1flowbase/page-runtime/browser` 剥离主线程编译器、按需延迟加载 `agentFlow`/`workflow`/`applications` 语言包，并保持 `shell-core`（`priority: 6`）高于 `native-host`（`priority: 5`），使 `shell` 降至 12 个分片（`396 KB gz`，登录后增量仅 9 个 JS 分片 / `315 KB gz`），`browse` 降至 16 个分片（`641 KB gz`）。
2. 关键路径压平与 BDP 受限预热：登录态启动阶段并发预取首屏根节点 `/blocks/{id}/code` 并静态扫描提前并行预热 `echarts`/`antd-style` 与 `compiler.worker.js`；构建期输出 `301 B gz` 的 `/authenticated-prewarm.json`，登录页表单挂载后按分层 SPT + $K=1$ 单流节拍预热，避免灌满外层 Nginx 下游 TCP 发送缓冲引发队头阻塞。
3. 容器内 `nginx.conf` 已启用 `sendfile`/`tcp_nopush`/`tcp_nodelay`/`open_file_cache` 并热更上线 `iqlater-ali:1flowbase-web-1`（无额外截止日期；容器重建前仍需打入新镜像）。

## 登录百毫秒切出与通用动态区块底座叶级解耦热更（2026-11-22 18）

用户于 2026-11-22 17:43 确认先完成纯前端通用动态区块底座热更验证，后端区块预编译缓存（`ephemeral` + DB）与批量装配接口暂不实施、留作后续新版本后端优化：
1. 预热与登录百毫秒切换：`useAuthenticatedPrewarm` 增加表单捕获阶段 `submit` `0 ms` 瞬停与未完成预热 `<link>` 移除，并按当前浏览器语言优先排序；`SignInPage` 先在干净信道完成 `/me`+`/session` 再切入零依赖 `AuthenticatedShellFallback` 工作台骨架；`auth-startup` 独立为 `12 KB gz` 控制面启动块，挂载后优先发出 `/navigation` 与 `/pages -> /tabs -> /blocks -> /code` 再加载路由大包。
2. 通用动态区块底座解耦：拆分 `overlay/` 下 `DatePicker`、`Select`、`Cascader`、`TreeSelect`、`Tooltip`、`Popover` 为独立单组件运行态（消除 `DatePicker`/`Select` 对 `antd-data`/`antd-form`/`antd-feedback` 的连坐）；新增 `native-modules/charts/runtime.tsx` 将 `@1flowbase/charts` 与 `@1flowbase/token-trend` 升级为挂载态按需门面（保留同步 `assertSafeEChartOption` 校验，未挂载或 `<Empty />` 空态 0 字节下载 ECharts）。已热更至 `iqlater-ali:1flowbase-web-1` 并通过线上无痕采样验证。

## 单调二态切屏、控制面严格先行与首屏二次瘦身热更（2026-11-22 19）

针对用户反馈的“多段加载动画跳变心智负担大”与“纯项目/容器内范围继续压缩首屏等待”，完成阶段四优化并热更上线 `iqlater-ali:1flowbase-web-1`：
1. 单调二态过渡（$M: 6 \to 2$）：删除 `AuthenticatedShellFallback`，登录提交后保持登录页按钮稳态 loading，先在空载 TCP 信道执行严格控制面门控（`prepareAuthenticatedLoginTransition`：`/navigation` + `/pages` 直接算出最终规范路径 `/route/pages/{id}` 跳过 `/` 与 `/route` 连跳，继而拉完 `/tabs`、`/content`、`/blocks` 并通过 `startup-block-code-cache.ts` 同步去重派发 `/blocks/{id}/code`），再预加载真实 `AppShellFrame`、当前激活语言包与 `FrontstageWorkspacePage`，最后经 `history.replaceState` + `startTransition` 单帧原子切入真实工作台；`BlockUiLoadingShell` 移除高频 `active` 扫光。
2. 关键路径再减重 `-133 KB gz`：将 3 处 `BlockUiLoadingShell` 引用切至 `@1flowbase/block-renderer/loading-shell`（切断 956 行旧 JSON 渲染器桶泄漏，`native-host` 从 `128 KB` 降至 `65.29 KB gz`）；`application-resource-loader` 新增 `activatePrimary` 使中文冷启动首屏不再阻塞下载 `i18n-shell-en`（`-28.8 KB gz`）；`NativeBlockDatePicker` 及 `RangePicker` 改为挂载态按需加载（默认“24小时”视图 `antd-picker` 降为 `0 B`，`-64.5 KB gz`）；区块预热仅先行预热首槽（Slot 0）模块，避免页底 `Table`（`antd-data`）抢占首屏顶部卡片带宽。

## 即刻 thinking 过渡、预画网格防塌缩与 268.5 kB 重复流量根除热更（2026-11-22 20）

根据用户截图反馈，修复三处线上实测瓶颈并热更至 `iqlater-ali:1flowbase-web-1`：
1. 登录响应与过渡：`SignInPage` 在 `/me`+`/session` 返回（`~250 ms`）后即刻切入单一全屏 `AnonymousLoadingState`（`thinking`），由 `AuthenticatedStartup` 在该遮罩下执行空管道控制面直达（`/route/pages/{id}`）并等待壳层及 `frontstage-core.css` 完整就绪后一次性挂载真实工作台，消除登录页 12s 等待与无样式裸奔。
2. 预画网格防塌缩：`RenderPlanSlot` 禁止未就绪态（`loading`）触发 `ResizeObserver` 写回 `autoRows`，严格维持预画的 `320px` 栅格高度与清晰 4 行柔和骨架。
3. 根除 `-268.5 kB gz` 阻塞流量：将 `transfer` 归入 `antd-data` 并设 `antd-data (priority: 1) > antd-picker (priority: 0)`，切断 `<Table>` 对 `antd-picker`（`92.5 kB`）的静态连坐引用；移除 Worker 的无效 `<link rel="modulepreload">` 并将 `NativeReactBrowserCompilerPool` 默认 `maxWorkers` 设为 `1`，消除编译 Worker 被重复下载 3 次（`-176 kB`）。

## 三层同心圆渐进式分层（顶栏 → 左侧菜单壳 → 右侧画布）热更（2026-11-22 21）

针对用户指出的“顶栏与侧边栏不应被具体区块画布阻塞”的分层问题，完成三层同心圆拆分并热更上线 `iqlater-ali:1flowbase-web-1`：
1. 顶栏先行（第 1 层）：`AuthenticatedStartup` 仅等待 `AppShellFrame` 与当前语言包就绪即上屏，壳内子路由 `Suspense` 改为非 `fullscreen` 局部加载态，永不反盖顶栏；懒加载未打开的 `AgentFlowDockPanel`。
2. 左侧菜单秒出（第 2 层，`frontstage-shell` `16.7 kB gz`）：`FrontstageWorkspacePage` 将 `FrontStagePage` 改为右侧面板内 `lazy` 组件（懒加载设计态 `MovePageModal`），`scenario-chunk-planner` 拆出独立 `frontstage-shell`（仅 `16.7 kB gz`）与 `frontstage-canvas`（`81.8 kB gz`），并在 `manifest.ts` 建立构建期硬门禁禁止 `workspaceShell` 混入画布或 `page-runtime`。
3. 右侧画布独立渐进（第 3 层）：`frontstage-canvas` + `native-host` 仅在右侧内容面板内异步加载并保持 `320px` 预画卡片骨架直至单实例 Worker 编译完成。

## 1:1 同构工作台骨架屏与单调高度棘轮防回缩热更（2026-11-22 22）

针对用户实测反馈的“登录进去还是有点空白”与“画布大小出现先规划好 → 缩小 → 再放大重画”两项体验细节，完成阶段七收口并热更至 `iqlater-ali:1flowbase-web-1`（`ETag: "6b02f735-1ae3"`）：
1. 消除登录后空白帧：新增零外部依赖 `system-metadata-seed.ts` 与 1:1 几何同构 `WorkspaceShellSkeleton.tsx`（`full` / `below-topbar` / `canvas-only` 三模态：`56px` 顶栏 + `180px` 左侧栏 + `320px` 圆角白卡），复用登录页已拉取的 `publicSystemMetadata`（`1flowbase` + `/icon.svg`）以 `0 ms` 填充 `useSystemMetadata` `placeholderData`；修复 `FrontstageWorkspacePage` 在 `pageTabsQuery.isLoading` 阶段漏传 `isPageContentLoading` 导致 `PageCanvas` 短暂渲染 `<Empty />` 的空白闪烁。
2. 单调高度棘轮防回缩：`NativeRuntimeSlotSurface` 接线 `onInitialContentCommit`（在 `AntdStyleSurfaceProvider` 及内部 `Suspense` 完成首帧提交前维持 `<BlockUiLoadingShell />` 并隐藏 `0px` Shadow 容器），并通过 `observeApiCall` 追踪挂载期初始 `ctx.api` 状态（`loading -> initial_api_pending -> settled` + `5s` 兜底），在初始接口飞行期间强制 `ResizeObserver` 高度不低于 `320px`（$\hat{H}(t)=\max(H_{\text{DOM}}(t),320\text{px})$），彻底消除区块先缩成小 `Spin` 窄条再放大重绘的视觉弹跳。线上无痕实测 `/` `formReadyMs: 1878 ms`，`pageErrors: []`。

## SRPT 控制面空载分波与核心分包再瘦身热更（2026-11-22 23）

针对用户截图反馈的 `pages`/`i18n-shell-zh`/`shell-core` 并发拥塞至 `15s–18s` 长尾问题，按已确认平衡方案完成阶段八优化并热更至 `iqlater-ali:1flowbase-web-1`（`ETag: "6b030f5c-1ae4"`）：
1. 消除递归预热洪峰与 $t=0$ 队头饥饿：`sign-in-resource-hints` 将 `virtual:1flowbase-authenticated-preload` 从 `AuthenticatedAppRuntime.tsx` 改为 `AuthenticatedStartup.tsx`（避免 `<link rel="modulepreload">` 递归并发拉起 `342 kB` 阻塞提交，入口预热耗时 `1553 ms -> 372 ms`，6s 输入期预热完成分片数 `3 -> 10`）；`prepareAuthenticatedStartupMount` 先在空载信道跑完 `navigation + pages -> tabs -> content + blocks`（Wave 0），再发车壳层 JS（Wave 1），并将 `catalog?locale=zh_Hans`（`20.3 kB`）、`frontend-blocks`、`unread-count`、`events` SSE、`registrations` 延后 `1.5s`，以带时间戳 `initialData` 消除 `system-metadata` 重复请求。
2. 核心分包大幅减重：`i18n-shell-zh` 仅保留顶栏 4 个核心命名空间，从 **`30.1 kB gz` 降至 `2.55 kB gz`（`-91.5%`）**（`settings` `62 kB` 改为齿轮悬停/设置页按需加载，`notifications` 改为打开抽屉按需加载，`frontstage` 随第 2 层并行加载）；`shell-core` 下调优先级至 `5.5` 并剥离移动端 `Drawer`/`Tabs`、404 `Result` SVG、`Typography`、`TopbarNavigationDesigner`、`WindowWorkspaceWindow` 与 `LoadingState`，单包从 **`114.38 kB gz` 降至 `20.52 kB gz`（`-82.1%`）**；`frontstage-canvas` 懒加载设计态专属菜单/悬浮工具栏，从 **`82.30 kB gz` 降至 `55.0 kB gz`（`-33.2%`）**。

## 全栈公网数学优化与本地 Mock 公网基准合并 `dev`（2026-11-23 17）

用户确认先保持 `iqlater-ali` 线上站点当前热更效果不动，在本地构建可复现 Mock 公网（`scripts/node/page-debug/wan-benchmark/*`：单连接 TLS H2 令牌桶 `150 KB/s` + `60 ms` RTT + PostgreSQL v3 协议帧计数与 `3 ms` RTT）并完成前后端协同改造合并回 `dev`：
1. 后端：新增 `GET /api/console/frontstage/bootstrap` $\mathcal{O}(1)$ 并发可见性快照 + `ANY($3)` 批量取码、双层 ACL 遮蔽的 `source_sha256`、视口 `(y, x, order)` 优先 `code_limit`、跳过源码表查询的强 `ETag / 304`、有界 `gzip` 响应压缩中间件（严格豁免 SSE / WS / 无上界流）及 `API_STATIC_WEB_ROOT` `.br` 静态直出。
2. 前端：认证态决议后 `0 ms` 零 AntD 早期并发发起 `bootstrap + navigation`（守住 `SIGN_IN_BUDGET`）、跨规范化路径缓存 `ETag / 304`、`source_sha256` 直命 IndexedDB 编译产物免拉源码免启 Worker、单飞 `Blob` URL Worker 按需懒启动、剔除非激活语言预拉与 `2500ms` 备用语言强拉。同物理参数（20 区块）冷启动首屏可见 `11,662.6 ms -> 5,495.7 ms`，热刷新 `4,036.1 ms -> 917.3 ms`（`588 KB -> 17.7 KB`）。线上 `iqlater-ali` 容器尚未发布本阶段前后端改动，待用户指定时同步发布。
