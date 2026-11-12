---
memory_type: project
topic: paipan-chat-command
summary: Issue #2821 已按半机预算编译成功并更新本地7800/3100；真实聊天事件创建/读取/编辑及Notification导入预览已通过，完整计算与发送仍待接通。
created_at: 2026-11-11 15
updated_at: 2026-11-11 17
decision_policy: verify_before_decision
scope: [communications, events, workflow, taichuy-calendar]
---

## Approved Scope

用户于 2026-11-11 15 确认：`/paipan --brith=2026-11-11 --time=13:09:00 --gender=男 --country=中国 --province=广东 --city=湛江市`。
六个用户参数全部必填；保留 brith 拼写；性别使用男/女；地址分成国家、省、市，避免逗号与代码输入负担。
默认使用真太阳时；中国范围使用 UTC+08:00；城市参考坐标从地址解析取得；运限日期/时间使用执行时刻，并显式传给插件。
用户随后明确要求“确定，就这样开一个issue，开始吧”，授权聊天指令事件及相关工作流源码实现，覆盖此前仅浏览器配置的限制。运行态业务配置仍走本机 Chrome/Playwright；允许 Mock，暂不跑质量门禁；资源预算以用户 2026-11-11 最新明确的一半 CPU、一半内存为准，不新增依赖。用户未指定截止日期。

## Current State

- 本地应用草稿：`01a1c981-0084-74f3-8f87-3f57cb25a958`，名称“排盘报告”，未发布；六个输入及校验节点已保存，尚无地区查询、日历服务计算或原会话回复。
- 事件草稿：`01a1c97f-0575-77a8-92c1-40fcb7ff0123`，名称“排盘请求”，已停用；其出生规范化接口契约仅用于核对命令映射能力，不能当作已完成的独立排盘请求事件。
- 浏览器实测完整输入可返回 validated；无效日期返回 invalid。缺少 city 在变量绑定阶段停止，但友好的聊天补参回复尚未实现。
- 2026-11-11 16 用户质疑新增协议必要性后只读复核：命令后端明确要求 `interface_event.is_none()`，但当前创建 API 强制 `InterfaceEventBinding`；前端目标列表仅筛 origin=custom，错误提供了接口事件。此前选出生规范化接口事件不合适，不能通过扩展嵌套映射来修补。
- 既有命令 → 普通自定义事件 → Event 工作流运行链路已存在；建议只补齐普通自定义事件创建配置契约与目标筛选，参数字符串在工作流解析，不新增事件传输或微信协议。该诊断随后由用户确认，最终采用固定来源 Schema 的聊天指令事件，已获源码修改授权。
- 现有命令回复是事件发布/接收回执，不能把 Workflow End 输出当成微信已回复；工作流有现成 Notification sender，可优先验证向当前绑定微信渠道发送报告，不先新增自动回传协议。
- 证据：`tmp/test-governance/paipan-chat-20261111/review.md`；当前参数方案 mock：`tmp/page-debug/calendar-chat-config/approved-command-mock.png`。
- 湛江固定案例由既有排盘页面取证：13:09 民用时间 → 12:46:28 真太阳时，四柱丙午/己亥/己丑/庚午；这不证明工作流或微信发送成功。

## Implementation Stage

- Single Issue: https://github.com/taichuy/1flowbase/issues/2821 ，保持 phase:implementation；包含全部七个原 AC，未关闭。
- Root 在独立工作区 `/home/taichuy/.codex/worktrees/chat-command-events/1flowbase`、分支 `codex/chat-command-events-2821` 实施；原 dev 未切换，尚未 commit/push/merge。
- 源码已补聊天事件类型、固定四字段 Schema、创建/编辑、目录、持久化迁移与受保护字段契约、命令目标筛选、固定身份映射。原生模板导入器遗漏 Notification 类型也已补齐；真实旧运行态尚无此修复。
- 新 Mock Event Workflow：`01a1c9db-fa20-77b3-b625-78e6dcf61c2a`，未发布、无事件订阅。通过浏览器原生导入和三次实际 QuickJS 调试：完整六参、缺 city、非法日期。日历计算是固定样例，Notification 是模拟，均 sent:false；直接调试不证明事件投递。
- 真实接口 Event 草稿：`01a1c9e4-a02e-75ac-b2a8-ed25bc7c0889`，未发布、未执行。含地区→出生规范化→真太阳时→八字→紫微→报告→Notification；旧后端导入器将八个通知节点显示为未知占位，候选启动后需重新导入。
- 旧阶段固定8GB限额曾导致构建OOM；用户已纠正为半机预算，2026-11-11 17 最终在10 CPU/约17.4GiB上限内构建成功，Cargo用时2分36秒，systemd峰值8.1GiB。后端7800及前端3100已更新运行，真实事件创建/读取/编辑已验证。未运行门禁或测试套件。
- 真实日历 HTTP 接口要求鉴权；草稿引用应用环境变量 origin/Authorization，尚未配置专用受限用户 API key。未保存 root cookie、未创建密钥、未修改真实微信绑定、未发送微信。
- 源码示例与中英说明：`docs/architecture/event-center/examples/paipan-chat/`；工作区证据：`tmp/test-governance/issue-2821/review.md` 与 `browser/`。API7800及前端3100已按用户要求更新，7600观察样本未动。

## Continuation Boundary

六参数方案与上述源码方向已获批准，不重复询问。继续前复核工作区 diff、Issue AC、两份草稿和候选运行状态；预算内编译与本地更新已完成，继续做受限鉴权、真实绑定、计算与通知取证。Mock 通过不能结算完整业务链；不扩大微信协议、任意事件发布或秘密保管能力。

## Updated Runtime Evidence

2026-11-11 17 用户明确要求重新编译更新后，独立分支已快进到主分支最新基线 `16097cdc62b2bbbc2a0f82fc7e854781fd1786d5`，保留受管服务激活/升级修复；主工作区dev未切换、任务变更未commit/merge/push。

- 新后端PID2766646，前端PID2766926，从隔离工作区提供7800/3100，已通过正式启动/健康探针；旧后端二进制留作回退。
- 真实排盘聊天事件 `01a1c9fa-92ae-76ed-9b45-994125ebb3d7`，名称“排盘请求（聊天指令）”，已由UI创建并刷新读回，编辑保存200。四字段固定Schema和类型持久化正常。
- 前端编辑初始化时useWatch的undefined导致误用接口校验，已用entry.event_kind初始化并通过真实PUT验证；共享node_modules字体403已通过窄依赖目录allow修复并确认200。
- 原生真实工作流导入预览现为0缺失依赖/0未知节点；旧存储的八个占位节点不会因升级自动转换，尚需重新导入。未跑真实日历调用、未发送微信。
- 最新报告 `tmp/test-governance/issue-2821/updated-review.md`，构建 `build-half-current.log`，浏览器 `updated-browser/`。编译阻塞已解除；后续聚焦受限鉴权、真实绑定与完整调用链。原7个AC不缩小，Issue保持开发中。
