# 网关请求资源优化验收（Issue #2228）

状态：局部功能验收通过，主要并发负载CPU收益可重复；严格AC-003仍未验证，Root保持open，不宣称整体稳定性目标全部完成。

## 环境与证据身份

- 实机：taichuy-dev，Linux x86_64，20逻辑CPU，约23GiB物理内存；这是测量环境，不是部署容量要求。
- 主工作区 `/home/taichuy/git/1flowbase` 保持 `dev`；本轮基线源码 `464ce849a6e74826e48fe0731aa26046e04068ff`。并行进入dev的Vite改动 `c10bebb7b` 已保留。
- 隔离工作区 `/home/taichuy/git/git_worktree/gateway-resources-20261003`，`codex/gateway-resources-20261003`。产品修改冻结于 `65e96e65e53459783e700873006f3d73d1aa31ec`；`fe691ea61b7c6bbb8a837d973a4c44491c25d1ec` 另修复一行 cfg(test) 的历史导入路径，生产代码相同。
- 基线API SHA256 `1be8f4fef1fbb720f3f62f893db80ea6e990af5e415180fe6f806f78b7142fee`；实际对照和长测候选API SHA256 `bb27cd190c2e9ee60524f60724bb9a3f0fba6cf163b6af9574086e1b7c2aa1af`，均Rust1.98.1、release、默认features、同Cargo.lock与编译参数。
- 精确fe源码CI构建SHA256 `fb31d753adca114e592c6d4f86c36d9658e569baa92c42f45cdf770ba942ca50`，与65构建不同；不能用fe标签覆盖65原始运行证据。测试源码修复的传播集合不包含生产行为。
- 独立PostgreSQL18容器、mock DB及dev公开schema克隆DB；API随机loopback端口。7600观察进程未受测、未重启、未替换。原应用配置与原密钥文件未修改。

## 改动与资源边界

|资源类别|当前owner与生命周期|本轮变化|
|---|---|---|
|常驻|API路由/冻结registry、异步runtime、DB连接池、插件宿主及供应商子进程；与进程共存|不新增进程、线程池或常驻业务缓存|
|请求间共享|`storage-ephemeral` 的发布计划缓存：按compiled_plan_id复用 `Arc<CompiledPlanRecord>`，既有5分钟TTL；供应商路由目录另有现存TTL及失效机制|保留既有owner；不把持久化会话正文放入ephemeral|
|每请求瞬时|协议解码、Native输入、执行节点、供应商通信、增量事件与序列化、持久化fact事务|每个客户端fact复用已持有的同一flow行锁，少执行一次SELECT|
|完成后保留|用于replay的事件环及closed run、allocator保留页、DB事实及正文归档|事件点查只检查目标run；既有后台到期回收与TTL不变|

`client_trajectory.rs` 的初始 `SELECT ... FOR NO KEY UPDATE` 已持有同一行锁到该fact提交。本轮删除第二次相同锁查询；Step、每个Section仍独立提交，校验先后、错误、序列高水位、有效失败前缀均保留。未采用会在后续Section失败时回滚既有Step的原子批处理。

`local_runtime_event_stream.rs` 原来每次 `run(id)` 对所有保留run做purge和最早deadline扫描；现在HashMap点查并保留目标过期拒绝，期望查找成本由O(R)降为O(1)。后台scheduler和open/list/reveal全局清理保留；未宣称全部事件成本O(1)。

可量化模型：CPU/请求 = 固定鉴权/调度成本 + 输入字节解析/复制成本 + 语义fact数量×持久化成本 + 增量帧数量×投影/发送成本 + 分摊后台工作。原始转发还承担运行、账务、恢复与完整日志事实；不能用网络等待时长或瞬时CPU百分比代替计算量。两个改动降低重复工作，不删减产品职责。

内存 = 进程常驻 + 热共享资源 + 活跃请求瞬时对象 + 到期前replay数据 + allocator保留页。PSS/请求不能简单用进程PSS除以并发数；10秒drain也不代表所有replay TTL已到期。

## 可复现mock对照

固定128条历史item，正文合计8KiB或512KiB；256个增量，每个10ms。每格1次warmup batch、10个测量batch，C1/C8；每批末3秒drain。主要C8/512KiB执行B→C及C→B两对，独立重建mock DB。官方测量关闭perf。
14格总计560个测量请求，逐个校验256 delta、一次completed、零error和精确输出SHA256，全部通过；warmup另计。

|场景|基线API CPU ms/请求|候选API CPU ms/请求|基线DB CPU ms/请求|候选DB CPU ms/请求|候选测量窗口PSS峰值MiB|
|---|---:|---:|---:|---:|---:|
|c1-8k-a|539.00|494.00|1113.14|1038.66|202.84|
|c8-8k-a|278.75|265.88|639.30|635.38|293.43|
|c1-512k-a|612.00|490.00|1266.23|1055.69|263.03|
|c8-512k-a|339.38|314.25|788.75|753.45|539.95|
|c8-512k-b|329.25|312.38|771.08|749.53|530.41|
|c1-8k-b|515.00|539.00|1063.01|1113.60|202.52|
|c1-512k-b|608.00|586.00|1237.19|1268.53|265.27|

主要场景两对合并：API CPU均值334.31→313.31ms/请求（下降6.28%）；含每批drain为345.81→326.25ms（下降5.66%）。数据库workload均值779.92→751.49ms（下降3.65%）。插件约24ms/请求，未改插件。两对API下降分别7.40%和5.13%，方向重复。

候选C8/512KiB各批API成本落在287.50–341.25ms/请求；批均值变异系数分别4.85%与4.64%。这是每批总CPU除以8，不是每个并发请求的独立因果归因，也不保证未来上界。单并发8KiB候选390–660ms，仍有明显波动。

1秒CPU峰值：基线92.31%/95.36%单核，候选93.61%/93.48%。没有证明峰值消失或利用率恒定；在多线程进程上CPU百分比也不是单个线程利用率。

尾延迟和反向 CPU 样本全部保留：C1首轮两格TTFT p95约上升22%，倒序对照没有重复该首字延迟退化（8K为203.85→172.59ms，512K为333.56→210.66ms）。但C1/8K倒序API CPU上升4.66%，wall p95从3089.29→3602.93ms（+16.63%）；C1/512K倒序DB CPU从1237.19→1268.53ms（+2.53%）。倒序C8候选wall p95 3573.85ms vs基线3555.32ms（+0.52%），TTFT p95 513.44 vs508.68ms（+0.94%）。前一对C8尾部改善。10个batch的p95精度有限；支持总CPU改善，不支持“所有尾延迟都改善”。

API测量窗口PSS峰值：主要两对基线589.78/568.54MiB，候选539.95/530.41MiB。候选单并发窗口PSS增量（本轮峰值减本轮开始值）均值：8K首轮1.25MiB，范围0.246–2.266MiB；512K首轮13.864MiB，范围6.737–20.816MiB。C8/512K两轮平均每批增量77.685/73.508MiB。这是进程窗口差，不是单请求分配字节；allocator及replay过期可使保留差值为负。短样本不能据此承诺固定单请求内存或无长期增长。boot/plugin安装期间的idle样本与warm workload口径分别保存。

物理DB和WAL没有下降承诺：主要场景候选两格DB增长42.07/43.35MiB，基线41.55/42.42MiB，包含warmup及物理分配；WAL也略增加。事务、正文与日志完整性均保留，本轮不是存储缩减。

采样口径：API/插件 `/proc` CPU累计tick与PSS，100ms采样；DB使用独占容器cpu.stat，200ms采样，对齐端点距离约100ms以内；DB含所有backend和后台进程。最初两格8KiB基线没有missing_samples计数字段，不能将null表述为0。采样器CPU单列，psql观察子进程不计入该观察器CPU，但DB快照在workload窗口外。PSS为顺序采样，短峰可能遗漏。

## 真实长会话

真实客户端共运行33.655分钟，12个有效根turn，原始工具活动跨度32.110分钟。根模型实际为gpt-6-luna/max，真实子代理为gpt-6.1-sol/medium；原始rollout分别有58和5次exec调用及对应结果。显式compact后在同一根线程成功续接，两条active-turn指令按RPC接受顺序出现在原始Native user正文中。独立核对event/flow/body SHA256；完整原始正文中nonce位置9848<10095，与RPC接受顺序一致。

该线程族对应17个flow、81次完整capture（根74、子7次Native generate），dropped_count和persist_failed_count均为0；上述工具数、turn数、flow数和网络请求数口径不同，不能互相替代。没有error notification或已恢复重试通知。模型、effort及父子关系由客户端原始rollout与Native正文交叉验证。

原长测脚本的总判定仍保留为 **UNVERIFIED**，errors=[]：额外的“子代理mailbox强制中断正在采样的父请求并重连到新flow”场景没有被触发。子代理确实完成并在后续Native输入出现，但它到达的采样以工具调用结束，未满足该额外oracle。没有修改oracle或原verdict；QA按本次AC-005明确要求，分别核实真实子代理、compact续接、指令顺序及30分钟有效工作后给出任务专项通过，不能声称额外强抢占/重连已通过。

|资源口径|有效工作32.11分钟|完整客户端33.66分钟|随后drain 10.29秒|
|---|---:|---:|---:|
|API CPU秒|42.48|44.43|0.06|
|供应商插件CPU秒|5.90|6.19|0|
|独占DB cgroup CPU秒|79.57|83.14|0.25|
|API PSS起点→终点MiB|186.22→240.72|183.30→236.48|236.48→226.90|

有效窗口API平均为单核2.204%，1秒采样峰值28.97%，PSS峰值245.42MiB。这个长会话的请求节奏与并发mock不同，不能用低平均值宣称并发容量。drain后PSS未回到启动值，既有replay TTL为2小时且未改变；本次未证明整生命周期无泄漏。DB PSS不可用，不能叠加共享页被重复计算的进程RSS；独占cgroup内存原值另存。采样器自身CPU12.601秒（2041样本），不包含psql子进程。

这条具体聊天从客户端开始到drain结束，数据库物理增长 **31,252,480B，约29.8MiB**；其中heap 7,979,008B，索引5,644,288B，TOAST（含其索引）16,662,528B；对象文件增长0。WAL产生171,670,632B，约163.7MiB，单列为写放大，不能当作聊天保留体积。

按表职责拆分物理增长：正文/上下文15,548,416B；归档正文/帧目录6,258,688B；轨迹索引/引用1,507,328B；执行/账务/系统7,036,928B。表总量30,351,360B与整个数据库增长间剩余901,120B来自目录及FSM/VM等。数据页复用和分配会影响物理增长；这不是原始文本逻辑大小，也不是所有“30分钟聊天”的固定成本。

隔离克隆只为测试补充临时key和子模型声明/路由，仍使用原共享上游账号；原dev数据库和密钥文件不变。客户端测试结束时，既有stopOwned在2秒SIGTERM超时后SIGKILL了API，原cleanup曾记录plugin仍在；随后确切PID/start_ticks复核表明两者均已退出。保留原失败及后验receipt，不宣称优雅停机通过。

## 验证与证据

- 集中轻量测试137通过；不会因上下文交接重复制造新验证成本。
- source65 CI存储52/52通过，涵盖本次并发sequence与失败前缀测试；API全lib测试编译在历史导入路径失败，保留原失败证据。fe已修复该测试路径，新CI存储52/52、事件流32/32通过。
- 精确fe实时负例验证：正式tool完成→上游输出前失败→58段未done工具片段失败→同会话新请求恢复；typed恢复receipt完整，callback只消费一次，无半成品tool提交。实际业务取消在收到前缀后执行；迟到上游delta/done/completed不覆盖cancelled，不向客户端泄漏。HTTP请求/响应与正式日志route、DB解压归档逐字节一致，limit 2/3翻页无遗漏/重复，持久化水位连续。
- 精确fe日志读权限负例：无会话访问已有日志401；临时owner合法app/run 200；同owner把该run放到不匹配app路径404；随后session释放、API正常exit0。这只覆盖本轮读取边界，不是完整权限矩阵。
- 精确fe读取长会话真实日志：首turn和压缩后turn分别22页64步、17页50步，DB ID/sequence与正式接口一致，tool section和Native原始正文hash一致。
- 原始报告与日志：主目录和隔离目录的 `tmp/test-governance/gateway-resources-20261003/`；`comparison.json` 可由主目录同目录 `summarize.cjs` 从14格 `report.json` 重算。
- 可复用mock入口：`scripts/node/ai-gateway-concurrency/resource-benchmark/run.cjs` 和同目录README；RLS入口为 `responses-long-session-acceptance/run.cjs`，必须显式覆盖默认base_url。
- 基线构建CI：https://github.com/taichuy/1flowbase/actions/runs/37109806977
- 首候选构建与52项存储测试：https://github.com/taichuy/1flowbase/actions/runs/37111955859
- 精确fe修复候选CI：https://github.com/taichuy/1flowbase/actions/runs/37113820317

## 结论边界与集成

主要C8/512K负载的单位CPU收益可重复，相关功能和持久化完整性有证据支持；CPU峰值消除、所有场景无尾延迟转移、固定每请求内存和任意高并发容量均未证明。AC-003的“重复CPU收益”已证，“不转移为尾延迟退化”的严格部分未证，保留反向样本，不能把Root所有目标写成无条件完成。

集中QA结论：AC-001/002/004/006在证据边界内通过；AC-005按本次明确任务场景通过；AC-003严格条件未验证，因此整个原AC集合仍为UNVERIFIED。未发现确定功能回归或生产源码blocker。Root采纳QA对局部优化的集成建议，合入两项已验证改动；该决定不等于放宽或结算原AC-003。Root及其未完成性能条件保持open。

本报告随源码合入dev，精确最终提交及推送回执记录在Root #2228。受测产品源码为65e96e65e，精确CI/补充实时验证源码为fe691ea61，本文和聚合数据仅补文档，不改变产品运行代码。7800没有重启或替换二进制；源码集成不等于运行环境部署。插件未改动，无版本升级或云端发布。临时CI workflow不进入dev。


## 证据索引

同目录 `mock-measurements.jsonl`、`long-measurements.json` 保存14格聚合与长测资源/存储原值；`measurements.json` 保存关键receipt和聚合数据的SHA256，可避免只保留四舍五入结论。完整采样、原始rollout及私有请求正文保留在本机 `tmp/test-governance/gateway-resources-20261003/`，不提交密钥或会话正文。隔离worktree的正式测量和QA证据归拢到该目录 `assembly-evidence/`；专用数据库停用前只读导出，备份保留在权限受限的 `private/final-database-snapshots/`，不上传。

任务临时API、插件、客户端及独占PostgreSQL容器均由Root回收；原dev数据库和其他容器不变。这里的进程回收包含前述超时强制退出，不是优雅停机保证。
