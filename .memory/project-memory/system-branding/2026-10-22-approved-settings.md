---
date: 2026-10-22 01
decision_policy: verify_before_decision
---

# 已批准的系统品牌设置范围

用户已批准 Root 在 1flowbase 中实现系统设置，原因是需要通过后台简单表单自定义系统品牌与登录展示，而不修改代码。Single Issue 为 https://github.com/taichuy/1flowbase/issues/2596 。

范围为 Logo、系统名称、关闭登录左侧标语开关、系统说明、备案 HTML；公开读取接口位于 /api/public/system-metadata，管理端读取与保存保持认证及 operation 权限。备案位于登录页右侧 Powered by 1flowbase 下方，管理员自由填写 HTML，原样保存展示，不增加过滤、白名单、iframe 或编辑器；此前受限方案已被用户否决。2026-10-22 用户纠正：登录左侧标语仅允许开关，标题固定，不允许编辑；新增 hide_login_tagline 布尔字段替换误加的 login_title。隐藏固定标题与固定标语整个文字区域，保留左侧动画与登录表单。system_description 独立保存，不能替换固定标语。后台表单遵循参考图，桌面横向标签、输入区铺满可用宽度，移动端纵向排列。

截止为本轮实现与验收，不设置后续自动执行时间。已完成本地 API/数据库与桌面/移动浏览器验收，开发服务已重建，等待用户最终验收 Issue；后续状态以 git 和 Issue 最新证据为准。
