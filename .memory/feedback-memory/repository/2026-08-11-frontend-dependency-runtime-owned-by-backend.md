---
memory_type: feedback
feedback_category: repository
topic: 外部 npm 使用本地预构建的 Nginx 静态 Module Pack
summary: 本文原“后端/Nginx manifest 提供运行时 module lock”结论已被 #1871 supersede；当前 Frontstage 模块、exports、实现和内容哈希资产全部归前端构建，后端不再声明或解析 dependency lock。
keywords:
  - frontend-dependency
  - npm
  - bare-import
  - dependency-lock
  - runtime-resolution
  - ui-management
match_when:
  - 设计或实现低代码区块第三方 npm 包管理
  - 决定包版本属于源码、发布快照还是后台运行时
  - 设计依赖安装权限、构建制品与浏览器加载链路
created_at: 2026-08-11 16
updated_at: 2026-08-24 21
last_verified_at: 2026-08-11 16
decision_policy: direct_reference
status: superseded
superseded_by: "#1871"
scope:
  - web/app/src/features/settings
  - web/app/src/shared/code-block
  - web/packages/page-runtime
  - api
---

# 外部 npm 由本地预构建并通过 Nginx 静态交付

> 2026-08-24 superseded：用户已确认 #1871。Frontstage 当前允许模块、exports、实现、类型和 Shadow DOM 样式改由前端构建的显式 module registry 统一拥有；依赖 JS/CSS 使用 Vite/browser 内容哈希资产。后端不再读取静态 manifest、声明 `code_modules`、生成 `dependencyLock` 或分发新的 frontend assets。本文仅保留历史决策背景，不再直接指导实现。

## 规则

- 管理页面与导航统一命名为“外部 npm”，不要命名为“前端依赖”。
- 不提供后台在线安装、审核、发布、版本数据库或服务端 / 浏览器 package-builder。外部 npm 是部署运维能力，不是后台设置中的动态业务状态。
- 开发者在本地可信开发环境完成 npm 安装、依赖解析和 Browser ESM/CSS/资产打包，输出一个完整的扩展包归档；部署时把归档解压或同步到唯一 Nginx `/external-npm/` 挂载目录。
- Nginx 目录中只保留当前扩展包；不设计历史版本目录、历史资产保留或服务器端回滚状态。更新时整体替换当前包，必要回滚由运维重新上传外部保存的旧归档。
- 扩展包内部仍包含自动生成的 `manifest.json`，仅用于描述当前包包含的 npm 模块、导出与资产，不代表版本历史。Rust 后端不保存、不投影、不代理这些包。
- 若保留“外部 npm”设置页，它只能读取静态 manifest 做只读展示，不提供安装、升级或删除操作。写权限属于服务器文件部署权限，不属于应用后台权限。
- 低代码源码只使用普通 bare import，例如 `import dayjs from 'dayjs'`；源码、模板和区块发布内容不记录也不要求作者选择包版本。
- 包名对应的当前启用版本、完整性摘要、导出和静态资产 URL 由 manifest 提供。替换 manifest 后，下一次运行解析到新制品，不要求改写源码或重新发布区块。
- `dependencyLock` 可以在编译产物和浏览器缓存身份中携带 manifest 投影的精确版本与摘要，但它不是用户内容、数据库对象或发布内容真值。
- 浏览器只从同源 Nginx 获取预构建的 Browser ESM、样式、支持资产和必要类型制品，不获取 npm tarball、依赖树或 `node_modules`。用户 TSX 的 Worker 编译与第三方模块的 Module Registry 加载保持分工。

## 原因

外部 npm 的必要复杂度是 npm 安装与浏览器打包；把它放在本地开发工具链可以复用成熟 Node 工具，而不把 Node 进程、构建任务、上传协议或版本状态引入生产前后端。运行时只需要读取当前扩展包的模块清单，不需要服务器维护发布历史。把版本复制进低代码源码仍会形成两个真值；浏览器直接处理原始 npm 包也仍会泄漏 CommonJS、条件导出、传递依赖、CSS、Node builtin 和 native addon 等打包复杂度。

## 适用场景

- `/settings/ui-management/code-templates` 如增加“外部 npm”，只提供静态 manifest 的只读目录视图。
- Native React 低代码区块的编译、依赖目录、Module Registry、缓存和运行时失效设计。
- 外部 npm 的本地打包、单包整体替换、Nginx 挂载、静态 manifest 与浏览器加载 contract。
