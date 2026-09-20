---
title: "Issue #2090 运行日志三级投影修复交付状态"
memory_type: project
created_at: "2026-09-20 02"
updated_at: "2026-09-20 02"
decision_policy: verify_before_decision
status: active
tags: [issue-2090, application-run-log, projection, delivery]
---

# Issue #2090 交付状态（2026-09-20 02）

- 谁在做什么：交付会话在宿主 `1flowbase`（分支 `dev`）实现 issue #2090（修复三级运行日志：运行中输入可见、
  系统上下文与最终输出投影、增量分页），产物在 `tmp/test-governance/issue-2090/`，原会话负责独立审计，用户验收后关闭 issue。
- 为什么要做：只读审计确认非终态会删除消息投影、native 分支缺 system 上下文、有任务锚点时已保存 answer 被排除、
  空消息页导致面板停止刷新；同时要求复用现有游标、不改原始事实、不重做事件存储、不接管 #2085。
- 候选 SHA：`f1b135e7ad1cedf31398f6fec04c55e5004d858d`（未推送）。方案：`source_revision` 用
  `v<writer版本>:<事实水位>` 驱动按需重投影；上下文作为一等投影（`context_source`，独立于分页流）；
  `output_source` 区分 provider 输出项 / 已保存 answer / error / 无输出；持久化 answer 兜底只在“没有正式回答项”时出现；
  前端按 `sequence` 归并、`newest_cursor` 增量追赶；新增 `rebuild-run-log-projection` 有界重建 CLI。
- 关键决策：writer 版本写进 `source_revision` 前缀，换 writer 后旧投影不会被当作最新（本次 4→5 触发全量失效重建）；
  分页流排除 `context_source is not null` 的行，避免“最新 5 条”把系统上下文挤出可发现范围。
- 2026-09-20 07 用户反馈修正：系统提示词不再做成详情面板上方的独立折叠块，而是作为会话**首条消息**注入
  （`items` 头部、每页都带、按 `sequence` 排在会话条目之前）；沿用与其他轮次一致的卡片样式（共享 console 的
  `--system` 变体原本是居中无卡片，已在本气泡内局部改为列布局 + 卡片），来源改成卡片头部小字标签、不再拼进
  提示词正文；随后用户指出逐条来源标签会**误导用户**，故最终**不渲染任何逐条来源标签**（`context_source` 仍由后端
  契约返回，仅用于区分/排序上下文条目），并删除对应 i18n key。后端把上下文并入 `items` 并去掉单独的 `contexts`
  字段，前端去掉折叠块、只保留一行调用类型/输出来源说明。
- 2026-09-20 08 用户第三轮回执：会话范围切换从面板头部移到**会话下方的一个图标**（composer header 槽位，tooltip/aria-label
  随范围在“查看此会话/返回当前任务”间切换）；同时修掉该视图上暴露的缺陷——会话范围卡片 answer 曾把工具调用名
  （`exec_command`）拼进回答，现只聚合回答类条目（`read_methods.rs`）。
- 2026-09-20 08 用户第四轮回执：删除顶部“调用类型/请求类型/输出来源”说明行（与“未生成回答”提示重复，同时删掉对应
  7 个 i18n key；`output_state` 仍由后端契约返回，前端只用于预热/无输出提示）；会话范围切换图标从 composer 槽位改到
  **最后一条消息的操作行**内（与复制/日志/时间线图标同排，仅最后一条 assistant 消息显示）。
- 尚未完成 / 需要谁处理：AC-008 待用户验收；AC-007 只拿到「普通调用 + 一次成功续接 + 168s 非终态事实可见 +
  真实客户端 1570s 事实跨度」，缺少“单次 CLI 会话内续接成功后 >91s 新事实”的组合证据；`codex exec` 续接被
  网关以 `native_tool_output_configuration_mismatch` 拒绝的根因是客户端两轮 `tools` 描述变化，属既有推理/工具
  提交契约，已交回审计定界。
- 决策背后动机：本 issue 的验收真值只看正文 AC 与真实样本证据，不接受“编译通过/测试数量”代替行为证据；
  凡需要改变原始事实、权限或推理契约的事项一律交回定界，不自行扩大授权。
