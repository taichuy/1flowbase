---
title: 组织管理已确认方案
updated: 2026-10-06 20
decision_policy: verify_before_decision
---
用户于2026-10-06确认Root提出的平衡方案，并强调用户管理UI参考两张相同原型图复刻。Root在隔离worktree organization-management实现，主工作树dev不切换。计划真值GitHub #2294（后续状态查看Issue和代码，不将此文件作为进度账本）。
部门属于当前workspace；多部门且有部门时主部门恰好一个、必须属于已选集；旧用户可无部门；成员筛选含后代去重；直接加入部门的角色权限叠加当前活动角色，不继承祖先；离开部门仅撤销该来源；有子部门或成员禁止删除。部门为主数据源内建树模型并复用现有ordered_tree。
动机：保留当前活动角色切换与直接角色行为，满足加入部门自动授权，复刻左树右表用户管理并新增组织管理tab。无固定截止日期，验收证据完成后交付。

## 2026-10-06 delivery
Implemented and integrated dev/origin/dev 85e40ef6014a80b220701ae1159d8e9017c670ab. QA_PASS_WITH_WARNINGS, backend CI37441624353 (19pass), member6/dept4/style4 pass. Evidence main tmp/test-governance/organization, root#2294 awaiting user acceptance; delivery#2295/#2296. Existing ZIP/home/docs fixture failures remain explicitly unverified outside org scope.

Main dev services restarted successfully (web3100/API7800); browser-probe-main passed after integration. Assembly worktree removed, evidence retained in main tmp. Only this private memory is untracked; tracked source clean.

## 2026-10-07 已批准演进
用户明确选择parent_id+ltree派生路径/GiST平衡方案，底座统一维护，覆盖动态树/部门/块树。继续Root2294，Delivery2298；组织根浏览与新增入口、成员批量查询一并交付。允许释放无修改旧worktree；已释放gateway-section-storage-research（分支保留），assembly organization-ltree，基线f3c5c9411。验收采用隔离CI+本地UI，未通过前不合dev。

2026-10-07 增量已集成dev/origin/dev df815902c。底座三类树ltree路径+ARRAY表达式GiST；触发器维护，历史迁移使用临时非空missing default避免旧MVCC版本建索引失败，之后移除default。成员角色/部门批量查询；左树组织根与新增入口。CI37573101140 87pass，前端22/style4，QA_PASS。证据tmp/test-governance/organization-ltree，Root2294待用户验收。

主目录dev-up重启成功；本任务organization-ltree工作区已回收，截图/报告保存在主目录tmp/test-governance/organization-ltree。旧研究分支保留，其他工作区未修改。

2026-10-07 用户要求明确唯一“组织”根以及空上级归属。沿用固定虚拟浏览根，parent_id=null 表示根下顶层部门；选择器显示“组织”，不创建实体根记录。通用树模板算法更新范围仍为 ordered_tree，非部门专用。

2026-10-07 用户批准通用树分页平衡方案（Single Issue #2299）：取消任意256深度/1000结果/100搜索命中硬上限，max_depth可选；分页响应items/has_more/next_cursor，组织页面按需展开/续页/远程搜索，已有部门选择单独补查。底座承担游标/查询一致性，组织保留权限与人数角色投影。跨页实时视图，锚点变化显式失效；不建设长期快照或新缓存。动机是功能完整性优先同时避免整树响应，目标与验收以Issue为准，无固定截止日期。
