---
title: "插件 manifest 的源码使用文件引用声明"
memory_type: feedback
feedback_category: repository
created_at: "2026-11-10 14"
updated_at: "2026-11-10 15"
decision_policy: direct_reference
status: active
tags: [plugin, manifest, source, registration]
---

- 规则：插件 manifest 只声明注册元数据、源码文件引用和资源契约；组件挂载源码、模板实现与类型声明应保存在独立文件中，不由生成器重新内联到 YAML。
- 原因：用户指出日历插件的 YAML 内嵌源码明显不合理；只分离组件、模板与区块注册目标仍未解决声明与实现混在一起的问题。
- 适用场景：插件源码注册、manifest 协议、打包生成器与底座安装投影。文件引用必须由底座协议正式支持并随包交付，不能只把路径字符串填进当前表示源码文本的字段。
- 授权更新：用户在实现后明确要求“测试提交代码”，此前暂不测试的阶段限制已解除。文件引用方向已确认；具体代码版本查 Git，验证范围与未验证项查 `tmp/test-governance/frontend-registration-submit-20261110/qa-receipt.json`，不将源码/包检查等同于运行态重装验收。
