---
title: "一键认证入口避免重复点击"
memory_type: feedback
feedback_category: interaction
created_at: "2026-10-20 07"
updated_at: "2026-10-20 10"
decision_policy: direct_reference
status: active
tags: [authentication, ux]
---

- 规则：当用户已经点击 GitHub 等一键认证入口，直接发起该入口的认证动作；不再进入只有同一个登录按钮的中间页。需要输入的认证方式保留表单。
- 原因：用户指出当前选择 GitHub 后还需再次点击使用 GitHub 登录，增加了没有新增信息的操作。
- 适用场景：登录方式选择器、一键授权入口与其插件激活行为设计。用户已确认使用声明式入口动作（显示表单 / 发起外部认证）；本轮不开放任意点击脚本。

- 账号语义：GitHub 首次注册创建独立普通账号；已有用户先登录，再主动绑定 GitHub，不按邮箱自动合并。用户明确确认这一方式。首次资料按供应商可验证信息初始化，公开昵称不冒充实名，后续授权不覆盖本地编辑。
