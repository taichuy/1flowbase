# Rust 预热与 API 单元测试资源治理验收

日期：2026-11-04。代码比较基线：d77c22f2f9cef346fd188a9c41fefa223e62549b；已通过独立 QA 并合入本地 dev 的源码：6352c16d93f733c9d237020efc9ff3afcbf3d8ee。后续仅追加本报告与使用说明，源码度量固定到该提交。

证据目录 E：/home/taichuy/git/git_worktree/api-test-targets-20261104/tmp/test-governance/api-test-targets-20261104。下文 R 表示 E/resource-runner。原始失败、环境修正、机器证明、实际测试日志与 SHA 均保留。全部结论按明确的源码、配置及执行范围结算。

## 1. 完成结果

问题已落实为可运行的改动：公开 API 契约测试迁入原生 Cargo integration targets；剩余私有测试留在库目标；共享 fixture 维持资源所有权；选定 runtime helper 使用拥有 Future 的类型擦除；普通测试与预热中的 workspace 测试编译阶段采用一致 test profile；现有 Cargo wrapper 默认及时回收 jemalloc 的可释放 dirty 页。

默认范围原有 2,082 项测试逐项保留。533 项分配到 12 个独立目标，1,549 项原测试留在库内，库内新增 2 项回归。因此候选 core 为 2,084 项，实际成功执行 2,083 项，保留 1 项原有 archive ignore。另执行了 health 5 项、frontstage 11 项，以及 Root 专项门禁的 3+3+1 项。

最终独立 QA 对源码集成给出 PASS。主目录合并后，`api_config` 的首次构建与 44 项行为测试成功，随后 10 次缓存 CLI 调用全部成功。第一次四阶段预热在 dev-up build 被宿主内存保护停止，未到测试编译阶段，不能记为主目录完整预热通过。两项新增私有回归复用已成功的 worktree 默认库行为证据，没有在主目录再次触发大型库测试编译。

本机预算为 10 核 CPU、12,303,990,784 B 内存硬上限、swap0。20 个逻辑 CPU、24,608,571,392 B 物理内存的半机预算中，内存沿用略低于精确一半的既有父 cgroup 上限。编译并发与测试线程在 main 的本地配置均设为 1；CPU 硬配额仍为 10 核，编译器内部可以使用并行执行。

7600 保护进程的最终核对：PID 1429270、start_ticks 163262379 与合并前记录一致，启动时间为 2026-11-03 13:18:44.790 UTC，仍监听 7600，exe 指向 `1flowbase_gateway/api/target/debug/api-server`。本次没有推送远端，验证流程没有执行运行中服务的重启命令；同期观察到的主目录其他启动记录另列，不据此声称整机没有其他人操作。

## 2. 原因：任务并发、编译范围与内存回收是三个问题

Cargo jobs 限制同时运行的编译任务数量。一个很大的 api_server --test 仍是一个 rustc 编译目标，jobs=1 不会把它拆小；测试名过滤也只选择构建之后运行哪些用例。

原单包实测正好体现这个区别：cargo test -p api-server --lib --no-run --locked --timings，在 jobs=1、CPU 上限 10 核下，318.61 秒后触发可用内存保护而结束，退出码 143；采样 scope 最大内存 10,722,271,232 B，宿主最低可用内存 1,926,418,432 B，没有新增 OOM。该旧记录没有绑定 HEAD，feature 图与本次 workspace 图不同，采样最大值也不是内核 memory.peak。该环境、该样本表明仅 jobs=1 不足以完成，单独保留其口径。

真正的分配画像进一步定位到编译器内部。有效 heaptrack 注入捕获的峰值 requested-live 为 7,616,032,671 B，对应 RSS 9,310,019,584 B；其中一条大的存活分配为 322,750,080 B，经过 MIR BasicBlockData 的 RawVec 扩容、coroutine create_cases 和 MIR 分析。单态化收集阶段继续形成很大的工作集，问题在进入最终链接之前已经存在。

因此本轮没有以 RSS 增长判定泄漏。requested-live、RSS、allocator 保留页和 cgroup 内存含义不同；RSS 与 requested-live 的差额也不是全部可回收空间。cgroup 还包含文件缓存等开销。

对 allocator 的取证先确认实际绑定：本机 rustc 1.99.0 在主可执行文件中静态链接 tikv jemalloc 5.3.0。最初的 mimalloc LD_PRELOAD 虽被加载，没有接管该主程序的 malloc；普通 heaptrack preload 也没有形成有效拦截。这两条尝试不纳入有效 allocator 对比。随后 libheaptrack_inject 的记录才作为有效堆画像。

证据：原仓 tmp/test-governance/api-server-lib-serial；E/allocator-binding-correction.json、allocator-reclaim-assessment.json、heaptrack-symbolized.json、candidate-reclaim-comparison.json。

## 3. 具体改动与边界

### 3.1 先选择 Cargo target，再选择测试名

12 个原生测试入口挂载既有 leaf，并链接普通 api_server 库；没有把生产源码树复制进每个测试程序。迁移的函数及属性保留，私有白盒测试继续使用所属库模块。

| 独立目标 | 实际发现 | 实际成功 | 原有 ignored |
| --- | ---: | ---: | ---: |
| api_config | 44 | 44 | 0 |
| http_error_contract | 12 | 12 | 0 |
| extension_graph_contract | 8 | 8 | 0 |
| host_registration_contract | 13 | 13 | 0 |
| interface_coverage_inventory | 5 | 5 | 0 |
| api_application | 151 | 150 | 1 |
| api_identity | 69 | 69 | 0 |
| api_console | 45 | 45 | 0 |
| api_system_settings | 30 | 30 | 0 |
| api_mcp_management | 53 | 53 | 0 |
| api_documentation | 46 | 46 | 0 |
| api_extension_catalog | 57 | 57 | 0 |
| 合计 | 533 | 532 | 1 |

这项改变的直接收益是让定向测试选择较小的编译目标。剩余库目标仍然较大，全量预热仍会编译它；新入口没有绕过全量发现。

### 3.2 复用共享 fixture，保持最后使用者的资源寿命

test-support 默认关闭，通过测试使用的 dev self-dependency 启用。普通生产构造行为保持。测试数据库与临时目录的 owner 由 runtime services 持有，state/provider clone 继续持有该资源链，owner 字段在相关 backend/session 字段后释放。新增回归验证最后一个 provider owner 释放前资源仍然存在，释放后正常清理。

公开 ApiState 结构没有新增 feature 字段。integration fixture 使用生产 UUID artifact resolver；原库内单元测试保留其既有 path resolver。没有增加数据库 migration，也没有新增第三方 lock package。

### 3.3 减少选定 helper 的泛型传播

选定的兼容 runtime helper 使用 Pin<Box<dyn Future<Output = ()> + '_>> 持有 Future。不同 Future 的本体仍然需要编译，公共 runtime/block_on 调用链可以复用一个被擦除的 Future 类型。该优化只作用于指定测试 helper，代价包括一次 Box 分配和动态派发。

规范关注 factory 调用位置、非 Send Future、正常完成、panic 以及 Drop 时的运行时上下文。独立 fixture 的有限验证中，56 cases 与 5 probes 的可观察结果匹配；两个选定 stack mutants 保留检出，2/2；错误的 borrowed 变体被目标回归检出。抽取 helper 的原实现与最终 boxed 实现通过同一回归，候选实际库另执行新增回归。这些结果限于明示的缺陷集合。

在同一独立 fixture、同样 `codegen-units=256` 等配置下，`-Zprint-mono-items` 的 `MONO_ITEM ` 前缀输出记录从 10,365 行变为 2,428 行（减少 76.575%）；以 `MONO_ITEM fn tokio::runtime::Runtime::block_on::<` 开头的记录为 59 行与 1 行。计数包含本次 fixture 编译发出的依赖相关泛型条目，没有过滤 Tokio/std 命名空间。这是诊断输出行计数，不能称为去重后的唯一实例数。原日志存在相同打印行：整行去重为 1456→1054，`block_on` 匹配行去重为 4→1；pretty-print 也可能隐藏不同宏展开/generic 上下文，文本去重同样不能恢复编译器的唯一身份。这些数字仅描述该 fixture 的诊断输出，不外推整个 API 的编译时间或内存改善。

原始日志分别为 R/owning-erasure/00-compile-old/command.log 与 01-compile-boxed/command.log；SHA-256 分别为 b317b49a75e635207aa2d0f544b70197f9934221b3bfbfc7d7f5c59a695f1a0c、722fb9553b09621685df4b36451cfd4ab2c0722f5b42a296d94404c55d06a183。两个编译均 exit0，没有为本报告重新编译。

证据：E/owning-erasure 的 SPECIFICATION、STACK_SPECIFICATION、manifest、stack-results；R/owning-erasure、R/helper-regression-resume。

### 3.4 对齐预热与日常测试的有效 profile

workspace 的 profile.test 默认 debug=0，与预热一致。API package 的 line-tables-only 覆盖保留，debug assertions、overflow checks、panic unwind 等设置保留。

普通 --workspace --test api_config 在清除 CARGO_PROFILE_TEST_DEBUG 后，其 570 个递归 unit 的字段与依赖边签名全部存在于预热的 650-unit 图中，差异为 0。这个结论绑定被验证的 api_config 目标和当前依赖图。单包选择可能形成不同的 feature 组合；本仓旧单包对比曾出现 67 个 unit 差异。

测试 CLI 增加显式 --workspace --test 的组合，workspace 与 package 选择互斥，零匹配继续报错。普通命令由 Cargo 管理 CARGO_MANIFEST_DIR，不向整个 Cargo 进程强行注入某个应用包的路径。

### 3.5 让已释放的内存及时归还

现有 Cargo wrapper 只在 MALLOC_CONF 未设置时默认使用 dirty_decay_ms:0；调用者的显式值及显式空字符串均保留。arena、tcache、background thread 等其他设置保持原值。

该环境会被 Cargo 的后代继承，使用 jemalloc 的测试或 cargo run 程序也可能采用；它不替换系统 allocator。更积极的 purge 可能增加系统调用、缺页和 CPU 成本，单次阶段耗时也确实出现增加。此处选用它是为了降低可释放页滞留，让本机预算内的编译能够完成。

在同源码、同 rustc argv 的诊断对比中，单态化图结束 RSS 从 9,094,328,320 B 降至 8,512,323,584 B，约减少 6.40%；对应阶段耗时 72.35 秒到 82.37 秒。默认臂在 LLVM 完成前被保护中断，dirty0 臂完成。B 臂另开启 confirm_conf/stats_print 取证；最终普通预热只使用 dirty_decay_ms:0。两臂完整阶段缓存状态不同，所以按阶段端点分别报告。

## 4. “映射／投影”在本次验证中的含义

映射记录一个原测试迁移到了哪个真实 Cargo target，以及测试名称前缀是否变化。投影则对给定门禁过滤器或 feature，从原测试全集中取出它应该执行的具体身份集合。

机器检查的核心是多重集合相等，而不是总数相等：

    恢复原身份(新目标中实际发现的原测试) = 原 2082 项，每项重数为 1
    对每个门禁 g：恢复原身份(新命令选中的测试) = 原命令选中的测试

例如，原文件管理门禁选中 12 项，迁移后分布为 api_system_settings 的 11 项和 api_documentation 的 1 项。新门禁必须仍然选择原来的 12 个具体身份；保留同样数量却混入旁邻测试，会被拒绝。

这一核对证明测试身份和既有门禁选择范围守恒；函数/属性 SHA 及实际行为另作证据。日常执行继续使用 Cargo 原生目标和既有门禁入口；本轮分析用的清单、映射与 verifier 放在 ignored 证据目录，没有新增常驻测试运行框架。

规范、实现、验证器与结果分层如下：

| 规范 | 实现与取证 | 机器结论 |
| --- | --- | --- |
| 原测试恰好保留一次，属性不变 | 原编译器展开清单、候选真实 artifact list、源码函数/属性 SHA | 原 2082 一次；候选 2084；456 函数/属性证明与 7 整叶 SHA 一致 |
| 错误的迁移必须被拒绝 | 名字/重数/目标/ignore/panic/实际文件归属校验 | 9 个负例拒绝，1 个合法相对路径正例接受 |
| 原门禁不能漏掉迁出用例 | 实际 command builder、workflow 过滤语义及真实目标名单 | 23 个旧门禁、783 次原身份选择守恒；8 个错误路由反例拒绝 |
| parser 必须绑定正确二进制 | 原有 protected parser、实际完成的 CLI 日志 | 同一证据旧绑定拒绝、新绑定接受；错误 transcript 反例拒绝 |
| Root feature 保留原专项成员 | 原默认清单 + 源码 cfg 静态投影，对比候选实际编译清单 | 实际 1025 项、ignored 0，4 个错误名单反例拒绝 |
| Future 与资源生命周期符合指定边界 | Rust 类型检查、有限状态/probe、实际新增回归 | 所选完成/panic/非 Send/owner 场景与缺陷反例通过 |

Root 的静态基线计算是 2082−1053−17+13=1025；17 项来自原 feature 的三处明确排除，13 项为该配置下的 12 个 Root fixture 加 1 个原 packages helper。旧 Root 没有编译，其基线由静态投影构造；候选实际编译清单为 1012 项保留的生产模块测试加上述 13 项。实际行为另结算原 workflow 的 network_proxy 3/3、root_1805_http_node 3/3，以及恢复的 package 测试 1/1。

## 5. 实际执行与失败记录

默认库首次完整执行为 1,544 passed、7 failed、exit101。七项都在未变化的可信 origin fixture 前提处停止；相关五份测试/调用/worker 源文件与基线整文件 SHA 一致。补齐既有 CI 的 API_AUTH_PUBLIC_ORIGIN=https://auth-fixture.example 后，同一 SHA 的库测试二进制精确重跑这七项，7 passed、exit0。两次成功身份集合恰好覆盖全部 1,551 项库测试，原首次失败记录保留。

其余需要纠正的验证问题也保留了原记录：临时日志 parser 的跨行匹配错误；编译器相对路径包含 ../ 的等价拼写；外层预算标记干扰纯 mock wrapper 测试。最后一项仅在模拟测试进程中清除标记，真实 kernel cgroup 保持，六组 allocator 场景仍显式测试 active=0/1。

门禁迁移实测还检出两处旧 Node 期望：遗漏 API target 映射，以及 organization 日志数量仍按旧值检查。最终提交使用精确的目标/日志集合，并保留 PostgreSQL 旧检查、逐日志非零检查；最终 Node 组为 79/79、0 skipped。早先 77/79 日志保留。

独立 QA 先对旧候选 2042106 给出 FAIL，发现持久化门禁与 Root 专项挂载遗漏；修复全部装配后，全新上下文 QA 对 6352c16d9 给出 PASS。报告分别位于 E/final-qa.md 和 E/final-qa-6352.md。

### 5.1 主目录第一次预热的资源停止

6352c16d9 合入 dev 后，第一次主目录预热在 206.909 秒被宿主可用内存保护停止。fetch 已完成，正在 `api-server dev-up target`；后面两个 workspace 测试预热阶段尚未开始。该阶段实际 Cargo 目标是 `build -p api-server --bin api-server --features tikv-jemallocator/stats`，属于 dev profile；采样确认只有一个 rustc，但未捕获该进程的完整 argv，因此不把它写成 `api_server --test`。

本次 scope 内核峰值为 9,086,545,920 B，最大观察进程 HWM 为 8,109,056,000 B，最低宿主 MemAvailable 为 1,332,289,536 B。launcher 由保护程序终止，exit −15，Cargo 没有完成状态；父子 cgroup OOM/max 事件增量均为 0，受管残留进程为 0。

最后 195.310→204.473 秒，宿主 AnonPages 增加 3,520,106,496 B，而本 scope anon 只增加 8,278,016 B，scope memory.current 反而减少 946,774,016 B；主要减少的是已回收的 file 页。同期父 cgroup 除本 scope 外的余额没有数 GB 增长。这支持尾部压力突增来自本任务及其 Rust 父组以外，不能精确归因到某个具体进程，也不能把异步计数器差值当成严格物理内存分账。

主目录另一 Vite 进程于 08:26:17.350 UTC 启动，与突增时间段重叠；由于没有该进程当时的同步匿名内存序列，这只是相关性。guard 停止后，08:30:14.837 写出了与 dev-up 配置相容的 API binary，08:30:15.020 新主目录 API 进程启动。root 没有调用该服务启动流程；文件及 fingerprint 的存在也不能预先证明下一条 Cargo 命令一定 fresh。

证据：R/main-dev-validation；E/main-warmup-pressure-analysis.json、main-stop-host-memory-snapshot.json、main-stop-process-identity.json、main-post-stop-artifact-timeline.json。此次失败与 W 成功预热属于不同目录与缓存状态；W 成功轮的 dev-up 阶段约 0.3 秒，不能拿其后测试编译峰值冒充同阶段对照。后续先改为单独的 native `api_config` 验证与缓存反馈采样，保持原 guard 和所有资源上限。

### 5.2 主目录定向验证与缓存反馈

R/main-target-feedback 两阶段均 exit0。首次 `node scripts/node/test-backend.js --workspace --test api_config` 实际编译了所需依赖及普通 `api_server` 库，再执行 44 项配置测试：44 passed、0 failed、0 ignored、0 filtered。实测 payload 为 379.830793 秒，内核 scope 峰值 8,442,834,944 B（7.863 GiB），最大观察进程 HWM 8,086,642,688 B；GNU time 为 7,897,112 KiB。宿主最低 MemAvailable 为 2,840,981,504 B，父子 cgroup OOM/max 增量为 0，残留进程为 0。被动 argv 记录直接确认普通 `api_server` 编译不含 `--test`。

随后在同一源码 6352c16d9、同一主目录执行 10 次完整 CLI，每次均为 44 passed、0 failed、0 ignored、0 filtered，均无 `Compiling` 行。计时包含静态门禁、Cargo 调用及测试执行；源码核对在计时之外。采用 h=(n−1)p 的线性插值，P50=1.334475528 秒、P95=1.741550577 秒，最小 1.259396560 秒、最大 1.982345148 秒，总 CLI 耗时 13.866578466 秒。整个采样阶段内核峰值 310,128,640 B（295.762 MiB），最大观察进程 HWM 168,550,400 B（160.742 MiB），同样没有 OOM/max 事件及残留。

首次构建并不便宜：原生小目标仍依赖普通 API 库。上述缓存反馈证明主目录日常定向入口可用，不等于冷编译只需一秒，也不是 PR/CI 延迟。第一次全预热失败之后，没有在相同宿主余量下反复触发完整大型库编译；继续保留 worktree 完整预热及实际行为的有效证据。两项新增私有回归在 R/candidate-tests-v2/17-remaining-library-execution/command.log 中均有精确名称对应的 `... ok` 记录。

证据：R/main-target-feedback、E/main-feedback/summary.json 与十份逐次日志、main-api-config.time、main-cached-feedback.time、main-target-passive-compiler-argv.jsonl。

## 6. 指标与测量口径

| 指标 | 优化前/参考 | 完成后的实测 |
| --- | --- | --- |
| 单一库测试注册范围 | 原 2082 项 | 原库 1549 项 +2 新回归；533 项迁至独立目标 |
| 旧单包失败样本 | jobs=1，318.61s 后保护终止，完成样本数 0 | 与下行范围/缓存不同，不计算加速比 |
| worktree 四阶段预热 | 既有依赖缓存；dev-up 阶段约 0.3s | 总 payload 692.001s，全部四阶段完成 |
| 完整预热 cgroup 内核峰值 | 旧样本只有不同口径的采样值，单列保存 | 10,519,941,120 B，约 9.797 GiB |
| 预热命令子进程最大 RSS | 旧完整编译被中断 | GNU time max RSS 9,572,800 KiB，约 9.129 GiB；非所有进程 RSS 之和 |
| 最终 Root 专项编译 | 原 Root 清单为静态投影 | 560.134s；内核峰值 10,133,286,912 B；完成 |
| 同源码 mono 阶段结束 RSS | 9,094,328,320 B | 8,512,323,584 B，端点下降约 6.40% |
| 缓存预热 | 独立缓存状态 | 0.954s；属于缓存命中样本 |
| 受控验证的 OOM/max 事件 | 旧中断样本 OOM 0 | 最终成功批次父子 OOM/max 增量 0，残留进程 0 |
| 原身份与门禁选择保留率 | 2082 原测试；783 原门禁执行身份 | 分别 2082/2082、783/783，100% |
| 所选缺陷检出保留 | 独立 fixture 中 2 个 stack mutants；1 个 borrowed 缺陷 | 2/2 保留、1/1 检出；有限缺陷集合 |
| 本机 CLI 反馈 P50/P95 | 同命令、同缓存的有效基线 n=0；native 入口为本次新增 | n=10，完整缓存 CLI：1.334s / 1.742s；每次 44 项成功、无重编译 |
| GitHub PR 反馈 P50/P95 | 本轮基线样本数 0 | 本轮候选样本数 0；没有远端 PR 延迟数据 |
| GitHub Runner 总分钟数 | 本轮未触发远端作业 | 本轮新增远端作业数 0、消耗 0 runner 分钟 |
| 默认行为验收本地成本 | 单独计量，不作为 CI 数据 | 103.538 分钟 payload wall、66.737 CPU 分钟；含 CLI 必要构建与七项补跑 |
| 主目录首次四阶段预热 | 单列目录/缓存状态 | 206.909s 在 dev-up build 被宿主 guard 停止；scope 峰值 9,086,545,920 B，OOM 0 |
| 主目录首次 native 目标 | 含依赖及普通 API 库编译；不与缓存反馈作加速比 | 379.831s，44/44 成功；scope 峰值 8,442,834,944 B，OOM 0 |
| 主目录缓存反馈采样资源 | 十次串行 CLI 合为一个受控 scope | scope 峰值 310,128,640 B；进程 HWM 168,550,400 B；OOM 0 |

CPU 分钟是所监控任务 cgroup 的累计 CPU 时间，wall 分钟是这些串行 payload 的实际耗时。数据库服务等宿主既有服务的 CPU 不计入该 cgroup。上表旧单包、workspace、缓存命中和专项 feature 的范围不同，分别报告，不合成一个整体加速比。

2082/2082 与 783/783 是原测试身份及门禁选择保留率，不是业务断言覆盖率或任意缺陷检出率。缺陷检出结论只覆盖上表明确列出的独立 fixture 反例。

同一轮 v4 预热中，剩余 api_server --test 的观测 VmHWM 为 9,348.44 MiB；能可靠绑定的 11 个独立目标观测值为 354.23–804.46 MiB，最大的为 api_application。这些独立目标复用了普通库。小目标只有一到两次采样，其值为已观察的高水位下界；interface_coverage_inventory 缺少足够的 argv 绑定证据，未填数。详见 E/native-target-compile-observations.json。

## 7. 维护成本与源码规模

度量固定为基线 d77c22f2f → 源码 6352c16d9，不包含本报告及最后的说明补充。统计使用 Git numstat 的整文件分类，未把格式、注释、测试门控行都称为生产行为。

| 分类 | 文件数 | 新增 | 删除 | 净变化 |
| --- | ---: | ---: | ---: | ---: |
| GitHub workflows | 4 | 14 | 1 | +13 |
| 治理实现 | 7 | 65 | 29 | +36 |
| 治理回归测试 | 6 | 181 | 4 | +177 |
| Rust 测试与入口 | 43 | 417 | 232 | +185 |
| Fixture / manifests | 13 | 228 | 49 | +179 |
| 正常产品路径中的测试门控/桥接及格式 | 5 | 107 | 52 | +55 |
| 已有文档调整 | 2 | 101 | 0 | +101 |
| 合计 | 80 | 1113 | 367 | +746 |

治理不含回归测试时为 +79/−30，净增 49 行；包含回归测试时为 +260/−34，净增 226 行。本次为目标拆分、共享 fixture 和检测保留增加了维护代码，并非治理代码减量项目。旧 lib 挂载已移除，原测试没有同时复制保留两套执行入口。

运行数据没有新增持久化数据库结构。堆分析原始证据经单线程 zstd 压缩：21,459,234,893 B → 2,608,050,841 B；解压字节数及 SHA 与原文件一致后才删除未压缩副本，释放约 18.85 GB 临时磁盘。

## 8. 后续日常使用

在 /home/taichuy/git/1flowbase 根目录执行：

    node scripts/node/reset-rust-cache/cli.js --warmup-only
    node scripts/node/test-backend.js --workspace --test api_config
    node scripts/node/test-backend.js --workspace --test api_config --filter parse_bind_addr
    node scripts/node/test-backend.js --workspace --test api_application

预热的 --warmup-only 与默认入口使用同一 buildRustWarmupPlan、同一循环执行 fetch、API binary build、workspace bins no-run、workspace all-targets no-run 四阶段。它跳过停服和删除 api/target 的步骤。本次验证的是默认入口使用的同一预热阶段，保留依赖缓存与服务；没有执行默认清缓存及停服步骤。

api_config 等纯配置验证可直接使用目标命令。HTTP fixture 使用 API_DATABASE_URL 或仓库现有本机 35432 默认测试数据库；PostgresTestSchema 在独占临时数据库中建 schema，角色需要对应建库、schema 和扩展权限。完整剩余库回归还沿用既有 trusted-origin 与四个 SDK worker fixture 的准备要求。本次实际构建了 runtime-extension-sdk 的 notification_worker、notification_authorization_worker、managed_event_worker、managed_hook_worker 四个 example；命令与 Cargo JSON 见 R/candidate-tests-v2/01-sdk-worker-build 和 E/sdk-worker-build-v2.jsonl。临时验证没有读取产品 .env 或用户密钥。

数据库隔离的源码依据为 api/crates/postgres-test-support/src/lib.rs（W/MAIN 共同 SHA-256：d80c49985b961a8ed09c1efec028a4d7f6e91325e7593cf464356c9c0f4789b2）。它创建 `test_backup_<UUID>` 整库，在新库里创建 `test_<UUID>` schema 与 public.ltree 扩展，并让连接池保有 owner；最后 owner 析构时连接维护库 postgres，尝试终止临时库连接并 DROP DATABASE。清理失败会记录错误，不能把析构尝试表述为所有情况下都保证删除成功；角色也需要连接维护库的权限。

私有白盒仍需选择 --lib；其名称过滤继续编译整个库测试目标。新增公开 API 契约测试应放入对应原生目标，避免继续扩大单一库测试程序。选择一个目标只执行该目标；完整回归继续使用全量入口与专项 feature 门禁。

## 9. 关键证据索引

- E/main-source-integration.json：合并、配置备份和源码 SHA。
- E/qa-final-6352-evidence.json、final-qa-6352.md：全新上下文 QA 的独立结论。
- E/postmerge-qa-6352-final.json、postmerge-qa-6352-final.md：主目录定向目标、十次反馈、资源与 7600 的独立复核；明确不将首次全预热资源停止记为通过。
- E/default-evidence-reuse.json、gate-test-expectation-fix-receipt.json：默认 Rust 证据到最终源码的可复用边界。
- E/inventory-before.json、candidate-artifacts-v2.json、inventory-verdict-v2.json：编译器清单、实际二进制及身份/属性证明。
- E/default-behavior-acceptance.json：真实逐项执行结果与发现清单的对账。
- E/gate-conservation-verdict.json、actual-protected-receipts-verdict.json：门禁选择守恒和实际日志回放。
- E/root-feature-artifact.json、root-feature-verdict.json、root-feature-execution-verdict.json：Root 实际产物、清单与指定行为。
- E/candidate-profile-coherence.json：有效 profile/递归 unit 图一致性。
- R/candidate-warmup-v4、candidate-warmup-v5、main-dev-validation、main-target-feedback：预热、主目录资源停止、定向成功及缓存反馈。
- E/main-feedback/summary.json：10 次实际 CLI 调用、分位数定义、每次结果及日志。
- E/main-warmup-pressure-analysis.json、main-post-stop-artifact-timeline.json：宿主压力与产物时间线的证据及归因边界。
- E/protected-7600-premerge.json、protected-7600-postvalidation.json：相同 PID、内核 start_ticks 与监听状态。
- E/candidate-source-metrics-6352.json：源码统计与逐文件分类。
- E/heaptrack-inject-compression-receipt.json：无损压缩的原/解压 SHA 与压缩文件 SHA。

堆原始内容与解压内容共同 SHA-256：97dcd276dbf6baba860dd0b52817339e5f0d2b51970af5029e107ce10a6ed5d8。

## 10. 一手资料与证明范围

本次判断优先使用当前源码与 manifest、真实 rustc argv/ELF allocator 绑定、编译器展开描述符、Cargo JSON artifact、实际测试输出以及内核 cgroup/procfs 记录。独立 agent 对规范和有限模型、测试集合与最终源码分别核对；机器反例用于检验校验规则本身能够拒绝目标错误。

概念参考使用官方原始资料，读取日期为 2026-11-04：

- [Cargo targets](https://doc.rust-lang.org/cargo/reference/cargo-targets.html)：Cargo 编译目标、unit/integration 边界。
- [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)：profile 与 package override。
- [rustc Monomorphization](https://rustc-dev-guide.rust-lang.org/backend/monomorph.html)：泛型实例化和代码生成机制。
- [jemalloc manual](https://jemalloc.net/jemalloc.3.html)：MALLOC_CONF 与 dirty decay。在线手册为更新版本，本机 5.3.0 的实际绑定与选项生效以本地二进制和输出为准。

本地验收覆盖本文列出的源码、默认测试行为、原门禁选择守恒、语法/Node/parser 检查、已列专项实际行为及资源边界；没有把 23 条 workflow 都当作完整执行过。Root 全部 1,025 项的身份已核对，行为执行范围为原门禁所列用例和恢复 helper；全 workspace 非 API 行为、全部 feature 组合与远端 GitHub Actions 不在该行为结论内。有限 Future/probe 证明适用于明示的观察点和缺陷集合。所有失败历史和测量口径保留，供之后复查。
