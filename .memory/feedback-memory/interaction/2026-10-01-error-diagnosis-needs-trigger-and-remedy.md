---
memory_type: feedback
feedback_category: interaction
topic: 错误排查需要触发原因与可行修复
summary: 用户要求排查时，不能只转述恢复记录或因脱敏日志缺失就结案；应利用可用原请求、参考源码和对照重放查明触发错误，再说明修复方案。
created_at: 2026-10-01 21
updated_at: 2026-10-01 21
decision_policy: direct_reference
scope:
  - error diagnosis
  - AI Gateway troubleshooting
---

# Diagnose the trigger and provide a remedy

## 规则

错误排查交付需说明最初触发原因、错误传播机制和可行修复方案。已有授权与可用环境下，脱敏日志不足时继续核验原请求保真、直接调用和经过网关的对照结果，不将“原始消息缺失”直接作为停止条件。实际不能还原历史报文时，区分历史证据、当前复现与尚未验证因素；不编造确定性。

## 原因

用户纠正了仅报告 non_reproducible、重试/降级失效和原错误被脱敏的初步结论，要求继续找到具体问题原因与修复方案。重放最终确认 context_length_exceeded，改变了修复方向：应保留语义错误，增加传输重试无效。

## 适用场景

用户要求排查具体报错，且已有日志、原始请求或安全可用的复现环境。
