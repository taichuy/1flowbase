export default function UsageOverview({ ctx }) {
  const state = useReport(ctx, true);
  const r = state.report;
  return (
    <Section title="模型用量总览" state={state}>
      {r && (
        <>
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fit,minmax(160px,1fr))",
              gap: 24,
            }}
          >
            <Statistic title="请求次数" value={r.request_count} />
            <Statistic title="总 Tokens" value={count(r.total_tokens)} />
            <div>
              <Text type="secondary">累计费用</Text>
              <div style={{ fontSize: 22, marginTop: 8 }}>
                <Costs costs={r.costs} />
              </div>
            </div>
          </div>
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fit,minmax(140px,1fr))",
              gap: 20,
              marginTop: 24,
            }}
          >
            {[
              ["输入 Tokens", r.input_tokens],
              ["输出 Tokens", r.output_tokens],
              ["命中缓存 Tokens", r.input_cache_hit_tokens],
              ["缓存写入 Tokens", r.cache_write_tokens],
            ].map(([name, value]) => (
              <Statistic
                key={name}
                title={name}
                value={count(value)}
                styles={{ content: { fontSize: 20 } }}
              />
            ))}
          </div>
          <Text type="secondary" style={{ display: "block", marginTop: 16 }}>
            已记录 Token 用量 {r.usage_recorded_count} / {r.request_count}{" "}
            次请求；未计费 {r.unbilled_count}{" "}
            次。缓存分项按供应商记录展示，不重复计入总量。
          </Text>
        </>
      )}
    </Section>
  );
}
