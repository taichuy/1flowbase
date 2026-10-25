# 工作流运行时的资源所有权

## 运行时热路径的资源所有权与等价边界

资源优化以功能完整性为前提，只改变已有数据的查找和临时对象生命周期。
事件容量、required/ephemeral 分类、公开协议、错误、ACK、持久化及回收语义继续由原 owner 决定。
本节涉及的实现不增加数据库查询、写入、索引、事务或新依赖。

### 职责与修改入口

| Owner | 职责与代码位置 |
|---|---|
| `RetainedRuntimeEvents` | 拥有事件队列、字节数、terminal 元数据及派生索引状态；定义于 `api/apps/api-server/src/host_infrastructure/local_runtime_event_stream.rs`。 |
| `LocalRunEventStream` | 以 ring 锁串行化追加、关闭、裁剪与退休；保留策略及分页位于 `api/apps/api-server/src/host_infrastructure/local_runtime_event_stream/run_event_retention.rs`。 |
| `RuntimeEventPersistenceBatch` | 拥有待持久化事件和批量字节计数；位于 `api/crates/control-plane/src/orchestration_runtime/runtime_event_persister.rs`。 |
| 定时任务 consumer | 拥有领取后的任务与解析结果，维持执行及 ACK 顺序；位于 `api/apps/api-server/src/workers/workflow_schedule.rs`。 |

`RetainedRuntimeEvents::push` 集中维护追加产生的 bytes、terminal、ephemeral 数量和排序标志，再把事件放入队列。
普通 append 与 terminal-if-missing append 都调用该入口，避免两个写块分别维护派生状态。
调用者仍拥有 admission、sequence、last_event_at、close 和 broadcast 的原有顺序。
terminal-if-missing 的原子检查与关闭仍在同一个 ring 锁内，不能由资源优化拆开。
terminal reason 与 sequence 保持成对存在；缺少 terminal 的分支使用同一 push 后仍得到原来的 metadata。

### Ring 派生状态与同步不变量

`ephemeral_count` 精确等于当前 `events` 中非 required 元素的数量，不是容量，也不是允许丢弃 required 的依据。
`sequence_order_may_be_unsorted == false` 保证队列按逻辑顺序非递减；true 是保守标志，不要求数据当前一定失序。
正常追加的 sequence 严格递增；已有序列计数在极端整数回绕后可能破坏该顺序，退路保留原过滤语义。
push 在 admission 删除完成后检查当前 back；若 back.sequence 大于 incoming.sequence，就把标志置为 true。
标志一旦为 true，不通过普通删除或显式 trim 猜测“已经重新排序”并清零。

| 变更 | 数量与排序标志的维护 |
|---|---|
| Default / 新 generation | 空队列、count=0、排序标志=false。 |
| 两个 append 入口 | 由 push 对非 required 加一，并在入队前检查顺序下降。 |
| `remove_retained_event` | 仅在实际移除非 required 时减一；保留逐项饱和减字节和 terminal 重算。 |
| 显式 trim | 在既有稳定 retain 中统计保留下来的 ephemeral；保留原 bytes 与 terminal 重算。 |
| `retire_payloads_locked` | `mem::take` 后 bytes/count 归零；排序标志继续保留。 |

退休不清除排序标志，因为消费者取得的 retired backlog 仍通过同一个 run/ring 读取。
backlog 是原队列的稳定子序列，旧标志同样保护它；无需新增一套退休容器索引。
新 generation 使用新的默认状态，旧 writer、订阅者与旧 generation 的隔离关系不变。
所有派生状态与事件变更在既有 ring 锁内完成，不添加跨锁更新窗口。

### 保留策略：减少扫描，不改变 admission

`max_events` 与 `max_bytes` 继续约束诊断保留；required 不会仅因这两个诊断预算耗尽而被拒收。
它仍可能按原规则因关闭、退休或其他真实错误失败；本优化不改变这些错误来源。
压力存在时，算法继续逐个删除最老 ephemeral；没有可删除项时才进入原 required 豁免或容量错误分支。
ephemeral 追加失败之前已经删除的旧事件仍被删除，不能通过提前报错或回滚改变这一状态。
同时触及两个预算时，按删除后的事件数量保留原 event-capacity 优先于 byte-capacity 的判定。
字节数沿用每次删除的 `saturating_sub`；不能以重新求和替代而改变饱和状态的行为。

每次 make_room/trim 调用从逻辑索引零开始；删除 index 后从同一个 index 继续搜索。
旧后继在删除后移到 index，若从 index+1 续搜会漏掉相邻 ephemeral。
循环保持“search_from 之前全为 required”，因此后缀搜索与原全队列搜索选择相同的最老 ephemeral。
精确 count 为零时无需遍历，直接执行原来的无可删除项分支。
连续全 required 积压的保留策略判定由每次 O(N) 扫描降为 O(1)，不包含编码、锁等待或分配成本。
一次调用中查找谓词的累计扫描为 O(N)，但 `VecDeque::remove` 的移动和必要的 terminal 重算仍可能累计 O(N×K)。
N 为环中事件数、K 为删除数；不能把整个混合裁剪算法称为线性，也不能据此声称已消除全部二次成本。
选择续搜而不做每次全环 retain，保留了原本靠近队首的一两项删除的短路径。

### 按序分页与极端退路

`after_sequence` 为 subscribe、replay 和 live backfill 提供同一选择逻辑，调用者的 validate 与 decode 顺序不变。
排序标志为 false 时，用 `partition_point(sequence <= cursor)` 找到首个满足原过滤条件的逻辑位置。
再从 `range(start..)` 继续执行原 `sequence > cursor` 过滤，调用者按原 limit 收集和 clone。
不计算 cursor+1 或 start+limit，因此负游标、i64::MAX 游标、limit=0 和 usize::MAX 上限不引入新溢出。
limit=0 也不能绕过原 replay-cursor 校验；已有 expired 错误与校验自身的算术行为保持不变。
序列缺口无需连续填满；`VecDeque` 的物理 wrap 不改变 partition_point/range 的逻辑顺序。
排序标志为 true 时 start=0，完整保留原过滤行为，覆盖极端序列溢出产生的失序输入。
只比较 front/back 不足以替代该标志，因为多次溢出和删除可能让中间仍失序而首尾看似有序。

有序路径单页选择成本为 O(log N + K)，K 为本页返回量；原来的前缀重扫最坏为 O(N + K)。
P 页累计返回 B 条时，选择成本由最坏 O(N×P+B) 变为 O(P×log N+B)，decode 和通道成本不变。
失序退路保持原复杂度；新增驻留状态只有一个 usize 与一个 bool，实际结构大小由目标平台对齐决定。
不新增每事件索引、缓存或数据库访问，也不改变慢消费者的 required 交付与 retired backlog 生命周期。

### Persister 的 Display 计长与 drop 生命周期

`RuntimeEventPersistenceBatch` 实现 `std::fmt::Write`，write_str 只按 UTF-8 字节长度饱和累加并返回 Ok。
push 使用同一个 `serde_json::Value` Display，通过 `write!` 计长后继续保留原事件。
原 `Value.to_string().len()` 先创建完整临时 String；新路径不创建这份临时输出，序列化遍历仍然存在。
对 Display 发出的非负片段长度 b1…bk，逐段饱和加法等于对完整字节长度求和后的一次饱和加法。
中文、emoji、转义、数字和对象键顺序继续由同一个 Display 实现决定，不改用另一套 JSON 格式定义。
计长对象不提供失败的 I/O；其范围是合法 Value 的现有 Display，不推广为任意自定义 formatter 的错误等价承诺。
减少的是临时 String 的分配与 drop；batch 仍拥有事件及 payload，不能提前释放待持久化内容。
`take` 仍按原顺序清零计数并移交事件 Vec，后续持久化和事件释放由原调用链负责。
64 KiB 字节阈值、20ms 延迟阈值及其触发规则保持不变；写入内容和既有数据库调用路径保持，不新增表、索引或逐事件写入。
实际定时批次的划分仍取决于运行时调度，不能据阈值等价宣称所有执行的批次数恒等。

单条和批量持久化在 repository 成功返回之后显式 drop 临时写入输入，再进入 after-commit delivery 的 await。
失败分支仍先记录持久化失败并返回原错误；不会提前释放 repository 正在使用的输入。
这缩短数据库写入副本在 delivery 等待/重试期间的驻留期，不降低数据库写入阶段必须同时存在的数据量。
replay/live 循环只在 terminal 事件出现时复制确认所需的 `(sequence, event_type)`，避免为每条普通事件复制类型字符串。

### 定时任务的 owned payload

`WorkflowScheduleRunTask` 仍是 application_id 与 flow_run_id 两个 Uuid 字段，使用原 Deserialize。
consumer 把已拥有的 `task.payload` 直接移入 `serde_json::from_value`，不再深拷贝同一个 Value 树。
解析得到相同 UUID 或原错误；忽略字段随被消费的 Value 更早释放，解析结果继续由 consumer 持有。
task_id 是未移动字段，解析后仍用于日志、ACK 和 outcome，不需要保留整个 payload。
claim、invalid-task ACK、业务执行和正常 ACK 的位置及错误传播保持不变，不增加重试或确认动作。

### 数学与有限 native 证据

等价论证以 count 精确、排序标志保守及稳定删除为归纳不变量，逐轮证明相同选择、状态、终止和错误。
它不把有限枚举宣称为工具完成的普遍形式证明，也不替代服务并发、数据库或协议回归。
Python 标准库有限域覆盖 admission 1,221,534 项、trim 203,589 项及饱和计数 74,592 项。
其中主保留域为长度0–4、required/ephemeral、字节长度{0,1,3}及terminal有无的组合，覆盖9组预算与饱和边界。
分页覆盖有序/物理环绕模型 1,634,388 项、带退路乱序 89,840 项，并检查 26,257 次状态转移。
native Rust 枚举基线源码抽取的保留函数与候选算法，完成 2,850,534 项保留策略比较。
真实 `VecDeque` 分页完成 1,634,444 项，包含 12,291 种实际 wrap 布局；元数据/退休检查为 93,310 项。
同 Display 的 native fmt::Write 计长完成 1,372 项；这些检查均已通过，但不是完整应用构建。
未守卫二分的反例 `[i64::MAX-1, i64::MAX, i64::MIN]` 在 cursor=i64::MIN 时被明确检出并由退路保护。
证据包括空环、limit=0、任意缺口、i64 游标边界、required/ephemeral 混合、错误前删除和预算错误优先级。

### 现有 L1 / L2 / L3 门禁归属

分层与结果复用继续遵循 `docs/architecture/testing/README.md`，不新建运行时专用验证框架。
L1 使用 `scripts/node/test-backend.js` 的 package/target/filter/exact 定向入口提供局部快速反馈。
ring 生命周期与慢消费者回归归 `api/apps/api-server/src/_tests/runtime_event_stream/lifecycle.rs`。
退休与确认回归归 `api/apps/api-server/src/_tests/runtime_event_stream/recoverable.rs`。
容量及载荷 admission 回归归 `api/apps/api-server/src/_tests/runtime_event_stream/payload_admission.rs`。
persister 回归归 `api/crates/control-plane/src/_tests/orchestration_runtime/runtime_event_persister.rs`。
定时任务回归归 `api/apps/api-server/src/_tests/host_boot/workflow_schedule_worker.rs`，沿用原真实 fixture 与 ACK 断言。
L2 继续由 `.github/workflows/verify.yml` 及现有 PR 契约/集成集合承担；L1 通过不代替该层验收。
Cargo 命令与执行校验归 `scripts/node/verify/backend.js`，实际测试证据解析归 `scripts/node/verify/cargo-test-results.js`。
正确 owner/binary 的非零实际通过才是行为证据；编译成功、空 filter、全部 ignored 或邻接成功不能替代。
L3 继续由 `.github/workflows/quality-gate.yml` 的完整 CI/coverage owner 承担，冻结同一候选 full SHA 并保留阈值和失败条件。
API coverage 分片、control-plane/orchestration-runtime 等包的既有 owner 及完整用例并集不因本优化转移。
数学检查、源函数微基准和实际服务回归分别陈述范围，不能用其中一种结算其余层级。

### 微基准记录边界

before 的四个业务源文件已按 SHA-256 确认与 v0.5.8 发布输入及本次增量前 `0cfa6727d` 输入相同。
分页/保留基准执行源函数抽取；serde/任务基准使用真实依赖和实际解析类型，均按独立 before/after 进程取样。
记录同负载 checksum、输出数、样本数、CPU/墙钟与峰值 RSS；分配请求量及隔离函数时间另列，不冒充端到端收益。

业务候选：`aacc1c25529f2ef4dce8d7da4bd87efb3fa8a31e`。Linux x86_64，rustc 1.99.0；没有数据库负载。
源函数以 `-C opt-level=3` 构建；serde/uuid 直接链接工作区已有 debug rlib，同一 binary 内选 before/after。
每种场景每版 11 个独立进程，按 AB/BA 交替。P95 是该有限样本的 nearest-rank 分位数（n=11 时为最大值）。

| 相同输入 | 函数 P50：前→后 | 函数 P95：前→后 | 全进程 CPU P50：前→后 | 峰值 RSS P50：前→后 |
|---|---:|---:|---:|---:|
| 50,000 条、有缺口/物理 wrap；128/页，8 轮 | 127.467→4.566 ms | 134.055→8.323 ms | 134.839→15.584 ms | 10104→10100 KiB |
| 20,000 条 required；原 64 条/4 KiB 诊断预算 | 65.262→1.266 ms | 79.009→1.486 ms | 68.077→4.760 ms | 5944→5940 KiB |
| 64 条混合环；131,072 次一两项短裁剪 | 4.211→4.643 ms | 6.061→5.436 ms | 233.069→231.838 ms | 12608→12684 KiB |
| 约 1 MiB UTF-8/转义 JSON，16 次计长 | 304.692→294.073 ms | 323.030→312.953 ms | 313.242→300.211 ms | 6136→4580 KiB |
| 两 UUID + 约 1 MiB 忽略字段，16 次反序列化 | 2.240→0.041 ms | 2.412→0.060 ms | 7.975→3.856 ms | 5516→4396 KiB |
| 正常双 UUID 任务（JSON 110 bytes），16,384 次解析 | 8.182→8.512 ms | 11.369→12.001 ms | 13.901→14.649 ms | 2448→2456 KiB |

ring 的时间/CPU 采用微秒精度 wait4 那一轮；RSS 采用独立 GNU time 那一轮，排除 Python fork 的执行前峰值。
JSON CPU 样本关闭分配观察器，GNU time 记录实际子进程 RSS；两版输入、输出及 checksum 相同。
短裁剪函数 P50 增加约 10.3%（每次约 3.3 ns），其全进程 CPU P50 近似持平；不将该路径列为提速项。
第一轮独立 ring 采样的短裁剪 P50 增加约 1.9%，说明这条很短的路径受测量离散影响；不筛选最有利轮次。
分页和全 required 的 CPU 收益来自减少重复扫描；ring RSS 没有实质下降。
分配观察器单独执行相同 16 次 JSON 负载：计长窗口累计请求分配 67,108,736→0 bytes，额外峰值 2,097,152→0 bytes；
任务解析窗口累计请求分配 16,812,160→0 bytes，额外峰值 1,050,760→0 bytes。这些是 allocator 请求字节，不等于 RSS。
正常双 UUID 任务另取 11 个独立进程/版：函数 P50 增加约 4.0%，取值区间重叠，RSS 近似持平，不能据大忽略字段场景宣称日常任务 CPU 提速。
确定的所有权收益是每任务额外 5 次分配、729 bytes 临时请求量归零；16,384 次累计 11,943,936→0 bytes。
该固定正常 fixture 不消费兼容保留的 payload_bytes 参数，实际序列化大小为 110 bytes，不能读取旧配置字段把它归类为 1 MiB 负载。
这些结果覆盖源码函数和真实 serde 操作；网络、锁竞争、数据库、完整服务吞吐及尾延迟属于服务级回归的独立口径。
数据库持久化字段及同一事件集合对应的记录数没有增加，批量写入触发规则保持；新增内存派生状态为 O(1)/run。

本地 API monolithic harness 的一次构建触及主机 90% 物理内存保护，任务进程组被主动停止，未进入测试执行。
该次耗时 360.657 s，进程树峰值 8,636,232 KiB，cgroup OOM 增量 0；不在相同条件下重复编译。
本次范围内的业务行为回归已在 GitHub Actions 独立 PostgreSQL 环境验收，本地保留轻量模型和源码基准。

### 候选执行证据

[候选业务验收 40155306928](https://github.com/taichuy/1flowbase/actions/runs/40155306928) 绑定 `aacc1c25529f2ef4dce8d7da4bd87efb3fa8a31e`，所有步骤成功。
运行时事件/定时任务 60 项、persister/service runtime events 41 项及 Agent Logs 原筛选并集 196 项，共 297 项实际通过，失败和忽略均为 0。
原 13 个筛选器与临时业务验收的 4 个筛选器逐项匹配实际通过名称；这些非零业务证据不证明已启动 target 的集合或资源成本与基线相等。
受控日志反例对发布版保留 14/14 个已知拒绝行为，对本次增量前版本保留 23/23 个（含后来增加的 binary-owner 规则）；候选另拒绝全部 37 个合并漏测反例。
发布版 parser 尚没有 binary-owner 接口，不能给它虚构该规则的基线分母。Node parser 正式测试 10/10 通过。
临时接受/成本测量入口随验收清理，最终 workflow 只保留六个正式步骤及已有 artifact 上传。
这些是本次涉及行为与门禁的证据，不作为全 workspace / 全产品 L3 回归的替代。

### P4 成本验收：CI2 被否决与修复探针

证据包中的 `evidence/ci2-summary.json` 绑定候选 `d2e681dfbfba18cdf9d35c1bf32afe2286f87bb2`。
CI2 的 job 和旧版基准收据均返回成功，10 个批次各有相同的 196 项通过测试；资源审查仍否决该 P4 版本。
同一 runner、候选源码和热缓存上，发布配方与该候选配方各执行5次，得到以下批次指标：

| CI2 指标 | 发布配方 | 被否决的候选配方 |
|---|---:|---:|
| 批次墙钟 P50 / P95 | 114.654 / 122.963 s | 111.245 / 113.192 s |
| 子进程 CPU P50 / P95 | 83.880 / 86.050 s | 102.350 / 108.520 s |
| 子进程峰值 RSS P50 / P95 | 621888 / 622712 KiB | 1265896 / 1272684 KiB |

这里的 P95 是 n=5 的 nearest-rank 最大观察值；批次时间不是整个 PR 或 job 的时间。
墙钟略短不能抵销 CPU 和峰值 RSS 的明显回退，也不能把65→25次 Cargo 调用计为已经通过的资源优化。
问题在于候选把全部 filters 移到 `--` 后，改变了 Cargo 的目标选择，额外启动了零通过的 Doc-tests。
旧验收只比较实际通过测试身份，因此没有发现这个额外启动成本；原 CI2 原始日志与收据保留为被否决版本的证据。

修复命令保留首个真实 filter 的 Cargo 位置：`cargo ... "${filters[0]}" -- "${filters[@]:1}" --format=pretty --color=never`。
其余 filter 仍使用原生 libtest OR，验收器仍逐项验证完整数组，两个 PostgreSQL integration 组仍分别执行。
基准增加已启动 target 身份并集的对等检查，包含零通过 target，并继续核对通过测试身份及每个原筛选的通过集合。
不依据“默认目标”这一名称推断是否应启动文档测试，而以同一工具链下实际 launch/suite 证据判断。

`evidence/filter-probe-fixed.json` 的原生小fixture比较结果为 PASS，记录的 Cargo 与 rustc 均为1.99.0。
旧 alpha、beta 两条命令各通过1项，修复后单条命令通过2项；无关用例仍被过滤，缺失或新增的通过身份均为空。
旧命令的已启动 target 并集与修复版相同，均仅有 `unit/src/lib.rs`，双方都未启动 Doc-tests；零通过目标并集也相同。
该结果只验证这个fixture上的参数转发和实际目标/用例并集，不能替代真实仓库的资源验收；早期探针原始收据保持不变。

[修复版 CI3 40159066918](https://github.com/taichuy/1flowbase/actions/runs/40159066918) 冻结候选为 `ed90f7f9a5d7a8500b302db1eba25f4bf24320ba`。
该运行的十批实际用例及启动目标身份全部对等：每批196个唯一通过用例，20个唯一目标，包含零通过目标。资源结果仍否决五进程设计。

| CI3 指标（同 runner，各5次） | 发布配方 | 五进程候选配方 |
|---|---:|---:|
| 批次墙钟 P50 / P95 | 175.447 / 175.970 s | 151.266 / 154.063 s |
| 子进程 CPU P50 / P95 | 146.250 / 146.930 s | 171.550 / 172.140 s |
| 子进程峰值 RSS P50 / P95 | 622452 / 622696 KiB | 1091696 / 1131332 KiB |

CI3 已排除额外 Doc-tests，但 API 七个筛选合在一个进程时仍出现显著 CPU/RSS 回退。contracts 与 storage 的合并没有出现这一量级的回退。
因此最终分组保留 API 七次及 PostgreSQL 两次调用的进程边界和相对顺序，仅收缩原来相邻的 contracts 两次、storage 两次调用。
正式入口按六个步骤组织，展开为11次 Cargo 调用；两个 API 步骤使用原生 Bash 数组循环复用配置，各筛选保留独立日志及逐项验收。
这保留 API/PG 隔离，contracts/storage 的隔离变化由同源实际验收确认；不把所有进程隔离都描述成数学等价。
CI2、CI3 属于不同 runner 上的独立实验，各自只在同一实验内部比较前后，不用两轮的绝对耗时差异计算优化收益。

### 最终 CI4：行为保留与资源成本

[最终集成验收 40162445979](https://github.com/taichuy/1flowbase/actions/runs/40162445979) 绑定 `a4e40fdd967e3c18e5ad02e506d3a7ac55e71153`，job 成功。
该候选已合入主线 `909c053ba363df7d8eb10299080496dc11f15484` 的并行交付；四个本轮业务源文件、两份业务测试及 Cargo 日志校验器与首轮通过版本逐文件哈希一致。
13 份正式/临时业务日志实际通过 315 次、314 个唯一测试身份，失败和忽略均为 0；原 297 个 `(package, test name)` 全部保留，26 个当前筛选器逐项非空。
正式 Agent Logs 配方为 197 次执行、196 个唯一身份；runtime API 70 项、persister/history 48 项。
额外覆盖的17项来自已合入的 SSE、交付预约/取消、generation 和 conversation history 边界；数量来自真实日志，未以预估数量代填。
CI1 仅记录包与名称，跨轮继承据此核对；完整原生 target 身份对等另由下面的同源配方比较完成。

两种配方在同一 runner、同一候选 Rust 源码、已预热缓存和 PostgreSQL 环境下交替 AB/BA，各执行5次。
十个批次的通过测试身份、每个原筛选的实际通过集合、包含零通过 target 的启动身份并集均完全相同：196个唯一测试、197次执行、20个唯一 target。
原有 API 重叠用例继续执行；七个 API 和两个 PostgreSQL 筛选保留独立进程，contracts 与 storage 各合并相邻的两次调用。

| CI4 指标 | v0.5.8 发布配方 | 最终配方 |
|---|---:|---:|
| 每批 Cargo / 校验器调用 | 13 / 13 | 11 / 11 |
| 每批 target 启动次数 | 100 | 93 |
| 批次反馈 P50 / P95 | 207.951 / 228.591 s | 198.473 / 209.436 s |
| 子进程 CPU P50 / P95 | 107.160 / 107.880 s | 107.460 / 108.730 s |
| 子进程峰值 RSS P50 / P95 | 623264 / 623692 KiB | 623020 / 623880 KiB |
| 五批累计诊断执行分钟 | 17.444501 | 16.718425 |
| 五批累计子进程 CPU | 535.730 s | 538.100 s |

本轮观察到批次 P50/P95 分别减少4.56%/8.38%，五批诊断执行分钟减少4.16%；CPU中位数增加0.28%、累计增加0.44%，RSS基本持平。
逐轮配对墙钟为3/5改善，另外两轮分别增加约1.70%和0.71%；API组CPU也有轻微增加，不能把分位数改善描述成每轮稳定提速。
这通过了 API 合并导致明显 CPU/RSS 回退的修复验收，但不是门禁 CPU 降低的证据；保留进程边界后，收益主要是配置复用、减少 target 重启及本次反馈时间缩短。
P95仍是n=5的最大观察值，不能把该有限样本当成稳定的线上尾分布；两个版本区间有重叠。
RSS是串行执行的 Cargo/校验器子进程峰值口径，未包含 PostgreSQL service、runner系统进程或冷构建阶段；批次分钟不等于整个job或整个PR反馈。

本次CI4包含预热、业务验收和十批对比，总job执行49.516667分钟；CI1–CI4一次性取证合计133.683333分钟。
两组临时业务接受步骤及五对采样步骤在交付工作流中删除，正式入口恢复为六个步骤、11次Cargo调用；这些一次性成本不成为日常固定门禁。
清理临时步骤后的最终Node合同/解析器复验51项通过、0失败：测试报告265.378473ms、命令墙钟0.327747s、峰值子进程RSS73684KiB；原14/14和当前23/23已知拒绝行为均保留，新防护37/37。
历史真实pull_request事件中9次非取消`verify.yml`运行的反馈P50/P95为545/632秒、累计runner执行229.116667分钟。
历史PR样本与本次同源诊断批次分别计量；`verify.yml`配方本身未被本轮改变，不用诊断批次的百分比推算全PR加速。

本轮永久变更相对已合入的`909c053ba363df7d8eb10299080496dc11f15484`计量，其他并行交付不计入本任务收益。
四个业务实现文件净增46行；两份业务回归净增211行；治理代码净增251行，其中workflow +20、解析器 +24、解析器测试 +93、既有workflow合同测试 +114。
因此本轮没有实现总代码行数下降：减少的是重复追加块、热路径拷贝/扫描及重复命令声明，新增成本是派生状态维护和可执行的合同保护。
不新增第三方依赖、数据库表/列/索引或任意业务容量上限；本地既有Cargo jobs=2、test threads=2及incremental配置保持。
