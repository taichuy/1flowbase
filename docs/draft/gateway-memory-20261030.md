# GM-P2：Gateway 冷启动内存候选

基线2902fc97f3730a0f772a691cb5e0cc7404c16143。仅共享冻结registry不可变schema、注册批次内相同原序列化字节的validator。catalog为221契约、200组schema、21份重复validator。身份、指纹、reference独立；原key顺序、Value相等、预算、shape、Draft202012及错误先后保留。索引注册结束释放，Arc随最后持有者释放。无新依赖、全局缓存、容量上限、数据库/插件/回放TTL改动。

## 性能

同dev profile/features/settings、插件hash、disposable DB fixture及输入/输出hash；普通Jemalloc无CountingJemalloc/background probe，profiling=false。三对串行新进程，第二对反转顺序；ready后30秒无业务/管理轮询，取尾10秒中位数，未清系统页缓存。

| 指标 | 配对节省中位数 bytes | 最小节省 bytes |
|---|---:|---:|
| RSS |19066880|17960960|
| PSS |19078144|17828864|
| Anonymous/Pss_Anon |17430528|17272832|
| Pss_File（单列） |882688|556032|

48/48 mock流oracle通过，含首轮、连续轮、slow reader，检查delta数、输出/terminal hash、唯一completed和error。18正常请求/侧：API CPU P50/P95为1.62/2.16→1.35/1.95秒，总28.73→25.78秒；服务延迟P50/P95为2723/4059→2255/3499ms。有限样本未观察到CPU恶化，不宣称稳定CPU收益；CI未改，PR反馈收益未测。Codex自身经过7600，属于观察者负载。

同设置辅助probe配对：Rust live72543349→55339718（−17203631），allocated81618704→63717376（−17901328）bytes；不是inclusive栈相加。8请求后普通静默PSS节省仅3.63/6.27/8.33MB；本包通过的是冷启动门槛，不声称每个会话均节省10MB。

## 存储

原六轮物理增长A合计22978560、B25305088，+2326528 bytes/+10.1%，保留该观测。另一次同8请求配对，234表请求前后聚合，三次逻辑快照稳定约6.8秒，run/node/log/task unfinished均0。各表新增行数完全相同，总6005行（含telemetry），业务行1679；业务payload21413028→21412995（−33），全表live tuple2370292→2371385（+1093）bytes。没有规范化ID/时间或排除telemetry成本。

稳定配对DB物理增量8224768→8232960，**+8192 bytes**；public表/索引/TOAST合计7995392→8019968（+24576），其余物理差−16384。flow_runs合计+40960、node_run_details−40960，各表行数相同，dead tuple估计随更新时序变化。未复现新增业务行/正文写放大；原六轮缺少逻辑快照，不能逐页追溯其全部差异。heap单独记录，index/TOAST/forks为合计，dead tuple为估计，未精确扫描死版本。未删除业务数据或VACUUM FULL。

## 验证及限制

- managed contract11项、interface-runtime46项通过。API check --lib --tests通过：2:03.93、峰值5842108KiB。Root边界检查9/9，receipt在root-boundary/。
- **83/83已知拒绝用例保留**，不是83个mutants或全Rust形式证明。Root对原版/候选各3个真实source mutants（reference身份/schema预算/规范排序），同4个既有测试均检出3/3；两clean及恢复后clean均4/4，总约27.6秒、最大913684KiB。有限1048576状态模型只证明其抽象。
- 单体API lib-test codegen17:28被SIGTERM终止，峰值10699048KiB、system1684.24秒、993860 major faults，未执行目标测试，未重试。Root确认parent累计oom_kill=1既存；缺少本任务开始前独立采样，不归因为本任务OOM。
- 隔离候选经7800真实gpt-6-luna/max：文本、强制tool call的completed/usage/hash成功。合成call_id的409 native_tool_output_unknown、改变tool_choice的409 native_tool_output_configuration_mismatch保留为拒绝证据。配置一致auto续接仍失败；唯一baseline控制也在HTTP200后收到7800 response.failed/type=invalid_request_error，无code字段，转发前后message hash相同。候选脱敏trace收尾中；该链不记为通过，不归咎外部供应商，不扩展native_tool_resume修改。

7600未施加目标负载、信号、attach、重启或运行文件写入；7800未重启。自启资源finally回收。Cargo已释放，提交供Root独立QA；不合dev、不push/deploy。

本机完整证据：tmp/test-governance/gateway-memory/下implementation、comparison、storage-comparison、diagnostic的md/json；六轮raw reports、storage固定配对、real traces、root-boundary/receipt.json、root-mutations/report.json。无密钥/业务正文。主仓非运行target当前含诊断probe输出；运行均用独立已hash artifact，不把该target输出当普通baseline。
