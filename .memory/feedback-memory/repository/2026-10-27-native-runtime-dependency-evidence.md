---
created_at: 2026-10-27 00
memory_type: feedback
feedback_category: repository
decision_policy: direct_reference
scope: native packaging and backend deployment explanations
---

# 运行依赖解释必须对应当前消费者

规则：解释 1flowbase 原生打包时，明确唯一后端为 api-server，静态资源可以由它托管。插件由后端管理，不把“插件运行依赖”笼统说成还要部署其他后端；镜像包含 Node.js 只证明镜像携带它，不能直接推出用户运行产品必须安装或随包携带 Node.js。

原因：用户对“必要插件运行依赖”的表述提出纠正，并指出当前只有一个后端。此前表达混淆了服务拓扑、插件子进程与尚未核实的依赖，增加了方案理解成本。

适用：部署说明、原生发行、依赖裁减和跨平台评估。按当前源码的真实执行入口决定依赖，并将已确认、候选、未验证三者分开表述。
