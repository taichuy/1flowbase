---
title: "Docker 部署与打包复核先限定当前源码"
memory_type: feedback
feedback_category: interaction
created_at: "2026-10-22 00"
updated_at: "2026-10-22 00"
decision_policy: direct_reference
status: active
tags: [docker, review, scope]
---

- 规则：用户问 Docker 部署配置及后续打包镜像有没有问题时，审查当前 Dockerfile、Compose、构建输入、脚本和工作流；未明确要求时，不扩展为旧线上包或历史部署容器的排查。
- 原因：用户于2026-10-22明确纠正“不要看我线上之前打包的，我说的是docker部署和后面打包镜像有没有问题”；历史运行态不能替代当前打包链路的正确性判断。
- 适用场景：源码部署审计、后续打包流程复核和安全升级传播检查；明确要求线上验收时再核对实际部署。

- 发布版本边界（2026-10-22）：用户明确说明发版时会升级版本；不把“自动发布只在组件版本变化时构建”的既定规则当作待修缺陷，不在本轮 Docker 修复中改版本号或发布选择逻辑。
- 修复范围：用户批准补齐 vendor 构建输入、修正文档命令和排除私有环境/记忆文件；.memory 未排除是构建上下文风险，不能据此断言最终运行镜像已经包含记忆。
