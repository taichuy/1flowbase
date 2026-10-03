import { TokenTrendChart } from "@1flowbase/token-trend";
export default function UsageTrend({ ctx }) {
  const state = useReport(ctx, false);
  const r = state.report;
  return (
    <Section title="Token 消耗趋势" state={state}>
      {r &&
        (r.request_count ? (
          <TokenTrendChart
            points={r.trend}
            bucketLabels={r.trend.map((p) =>
              new Date(p.bucket_start).toLocaleString("zh-CN", {
                timeZone: "Asia/Shanghai",
                month: "2-digit",
                day: "2-digit",
                ...(r.bucket === "hour"
                  ? { hour: "2-digit", minute: "2-digit" }
                  : {}),
              }),
            )}
            ariaLabel="Token 消耗趋势"
            height={360}
          />
        ) : (
          <Empty description="这段时间没有模型请求" />
        ))}
    </Section>
  );
}
