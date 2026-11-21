---
memory_type: project
topic: IQLater 线上 Docker 登录入口修复范围
summary: 用户只批准纠正登录入口的旧发布覆盖，尚未批准完整性能改造；已恢复读取当前镜像登录文档。
created_at: 2026-11-20 20
updated_at: "2026-11-20 22"
decision_policy: verify_before_decision
status: deployed
---

用户于 2026-11-20 在当前聊天要求修复线上登录入口未使用已安装 Docker 镜像产物的问题。动机是让实际入口跟随镜像版本；此次不是对编辑器懒加载、语言资源拆分或外层缓存改造的授权。无另定截止日期。

Root 在该日完成持久化 Web 配置与运行配置的窄修改：`/home/deploy/git/1flowase/docker/web/releases/85ee08b2/live.conf` 的 `/sign-in` root 指向 `/usr/share/nginx/html`，旧目录只保留 hash 静态资源回退；仅热加载 Web Nginx，没有重启容器或触碰 7600。

后续操作前重新核对当前 mounts、运行配置和公网文档，不把本记录当成实时部署事实。证据入口为本地 `tmp/test-governance/iqlater-docker-entry-fix-20261120/report.md`。服务器备份/回滚入口为 `/home/deploy/git/1flowase/docker/web/releases/docker-image-entry-20261120/`。其余登录后性能问题仍为未实施方案，后续需明确范围。

## 权限缓存排障边界（2026-11-20 22）

用户要求本地复现和修复动态路由勾选回退，留待下一版本发布；本轮没有修改线上角色、数据库、代理或部署。只读查询观察到权限 GET 的 `X-Cache: EXPIRED → HIT` 且缺少 Cache-Control；本地受控缓存复现了全选落库8项却回显旧5项、后续保存覆盖之前选择。下一次线上操作必须重验响应头与外层缓存配置，不能假设代码更新会清除已有缓存。证据入口：`tmp/test-governance/role-route-permissions-20261120/report.md`。系统 root 豁免语义保持不变。用户未指定部署截止日期。
