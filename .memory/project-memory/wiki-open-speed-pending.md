---
title: "Wiki 主键读取已正式发布，缓存修复待用户重启"
memory_type: project
created_at: "2026-10-25 07"
updated_at: "2026-10-25 08"
decision_policy: verify_before_decision
status: active
---

- 用户明确要求立即更新 Block，不接受因独立缓存问题继续暂缓。主键读取已通过 1flowbase-gateway 正式发布到 Block 01a16e2c-e187-7776-8f48-da8435db641b，主键版本 revision 03849a4924c3009fa8437bdbcc86bcc53175df4b2f75bd34aebe4a6e2f31c113（后续移动优化版本见下）。不要再次把它描述为待发布或仅预览。
- 实际已部署页面（无源码拦截）点击 React、周末随笔、旅行计划，均只发送一次 get/{id}，请求 338/335/347ms，点击到标题 1059/838/823ms；快速切换旧响应忽略、503 后停止加载、单次重试、文件夹零请求均通过。证据 tmp/test-governance/wiki-query-diagnosis/primary-delivery/。
- 初始化防重复及选中状态修复先前已发布，初始请求 15 降到 5；本次进一步把每次文章点击 4 请求降到 1。
- 7600 禁止由助手重启。后端缓存修复 83e8805ec 已合并推送，但当前 PID 2410098 未重启。缓存问题独立待用户重启后验证：此前一次性记录 get A→PATCH B→get 仍 A→tree B，已清理记录。证据 tmp/test-governance/wiki-query-diagnosis/cache-dependency.json。
- dispatch 额外延迟仍需独立定位：直接 get 6–11ms，dispatch 通常约 350ms，曾出现 1.29 秒；源码存在每次重建完整 capability catalog。截图 5–14 秒尚未复现，不能宣称解决。
- 截止用户下次重启确认，后续只需验证缓存正确性及继续定位分发延迟；无需重新发布主键读取。前端源码导出与 deployment.json 已同步，旧 pending-deployment.json 状态已标为 published_primary_read_user_requested。

- 移动请求放大已修并正式发布：当前 source_revision 76c60722569e2ea606a720c4689b47181513c1dc21787f444a9c50bae97e3422。move 成功后只读目标直接 children 或 roots；依据后端返回替换目标同级集合和顺序，其他分支保留，不再刷新标签/全树。当前数据每次移动从 6 请求降至 2（超过100个同级时正常分页）。原生 move 仅返回 moved:true，因此保留一次权威读取。
- 已部署页面实际拖拽验证同级排序、跨目录、回根、文件夹携带子树，每次2请求；循环被前端拒绝；503后重试只读1次，不重复移动；刷新持久化通过。临时6记录全部清理，证据 tmp/test-governance/wiki-move-speed/deployed-result.json。本轮未修改仓库产品源码/未重启7600。

- 后续用户要求参考 Block 01a042be-1ba8-7c00-acfb-b7f1168f4797（page 01a042be-14d2-76e1-b35b-924c3d2fb024）的 Ant Design Splitter。已正式发布原生 Splitter 两栏，当前 revision be7a69f65d5a33740f023f915b6b54fd2303fdde0238bb7494c7ab524085f1a3。默认左栏340px，左右拖拽，阅读/编辑保持宽度，窄屏上下排列；通过 ctx.ui.sizing.reportIntrinsicSize 上报实际高度以适配宿主。真实浏览器验证340→477→394px、编辑保持、拖动零业务请求、单篇1请求、移动端布局。证据 tmp/test-governance/wiki-splitter/。
