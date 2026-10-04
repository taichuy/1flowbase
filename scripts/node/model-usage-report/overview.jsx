function MetricIcon({ kind }) {
  const paths = {
    requests: "M6 3h8l4 4v14H6z M14 3v5h4 M9 12h6 M9 16h6",
    tokens:
      "M4 6c0-4 16-4 16 0s-16 4-16 0v12c0 4 16 4 16 0V6 M4 12c0 4 16 4 16 0",
    cost: "M12 3v18 M17 7c-1-3-10-3-10 1 0 5 10 2 10 7 0 4-9 4-10 1",
  };
  return (
    <svg
      width="24"
      height="24"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d={paths[kind]} />
    </svg>
  );
}
const compactTokens = (value) => {
  if (value === null || value === undefined) return "—";
  for (const [scale, unit] of [
    [1e9, "B"],
    [1e6, "M"],
    [1e3, "K"],
  ]) {
    if (Math.abs(value) >= scale) return (value / scale).toFixed(2) + unit;
  }
  return count(value);
};
function SummaryMetric({ title, icon, color, background, value, children }) {
  const { token } = theme.useToken();
  return (
    <article
      aria-label={title}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 14,
        padding: "16px 20px",
        minWidth: 0,
        minHeight: 104,
        boxSizing: "border-box",
        border: "1px solid " + token.colorBorderSecondary,
        borderRadius: 16,
        background: token.colorBgContainer,
      }}
    >
      <span
        aria-hidden="true"
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          width: 44,
          height: 44,
          flexShrink: 0,
          borderRadius: 10,
          fontSize: 22,
          color,
          background,
        }}
      >
        {icon}
      </span>
      <div style={{ minWidth: 0 }}>
        <Text type="secondary" style={{ fontSize: 13 }}>
          {title}
        </Text>
        <div
          style={{
            fontSize: 26,
            lineHeight: "32px",
            fontWeight: 650,
            fontVariantNumeric: "tabular-nums",
            color: title === "累计费用" ? color : token.colorText,
          }}
        >
          {value}
        </div>
        <div
          style={{
            fontSize: 12,
            lineHeight: "18px",
            color: token.colorTextSecondary,
          }}
        >
          {children}
        </div>
      </div>
    </article>
  );
}
export default function UsageOverview({ ctx }) {
  const state = useReport(ctx, true);
  const r = state.report;
  const { token } = theme.useToken();
  return (
    <section aria-label="模型用量总览" style={{ minWidth: 0 }}>
      <Flex
        justify="space-between"
        align="center"
        wrap
        gap={8}
        style={{ marginBottom: 12 }}
      >
        <Text type="secondary" style={{ fontSize: 12 }}>
          {state.range
            ? `${dayjs(r?.started_from || state.range.started_from).format("MM-DD HH:mm")} — ${dayjs(r?.started_to || state.range.started_to).format("MM-DD HH:mm")}`
            : "用量统计"}
        </Text>
        <Filters state={state} label="模型用量总览" />
      </Flex>
      {state.error && (
        <Alert
          type="error"
          showIcon
          title="暂时无法读取报表，请重试"
          action={<Button onClick={state.retry}>重试</Button>}
          style={{ marginBottom: 12 }}
        />
      )}
      <Spin spinning={state.busy} description={state.busy ? "正在更新" : undefined}>
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fit,minmax(min(100%,280px),1fr))",
            gap: 16,
          }}
        >
          <SummaryMetric
            title="总请求数"
            icon={<MetricIcon kind="requests" />}
            color={token.colorInfo}
            background={token.colorInfoBg}
            value={r ? count(r.request_count) : "—"}
          >
            所选范围内
          </SummaryMetric>
          <SummaryMetric
            title="总 Tokens"
            icon={<MetricIcon kind="tokens" />}
            color={token.colorWarning}
            background={token.colorWarningBg}
            value={
              <span title={r ? count(r.total_tokens) : ""}>
                {compactTokens(r?.total_tokens)}
              </span>
            }
          >
            <div
              title={
                r
                  ? `输入 ${count(r.input_tokens)} / 输出 ${count(r.output_tokens)}`
                  : ""
              }
            >
              输入: {compactTokens(r?.input_tokens)} / 输出:{" "}
              {compactTokens(r?.output_tokens)}
            </div>
            <div
              title={
                r
                  ? `缓存命中 ${count(r.input_cache_hit_tokens)} / 缓存写入 ${count(r.cache_write_tokens)}；缓存分项不重复计入总量；已记录用量 ${r.usage_recorded_count}/${r.request_count} 次`
                  : ""
              }
            >
              缓存命中: {compactTokens(r?.input_cache_hit_tokens)} / 写入:{" "}
              {compactTokens(r?.cache_write_tokens)}
            </div>
          </SummaryMetric>
          <SummaryMetric
            title="累计费用"
            icon={<MetricIcon kind="cost" />}
            color={token.colorSuccess}
            background={token.colorSuccessBg}
            value={
              r
                ? r.costs
                    .filter((c) => c.total_cost !== null)
                    .map((c) => (
                      <div key={c.currency_code} style={{ fontSize: 24 }}>
                        {Number(c.total_cost).toLocaleString("zh-CN", {
                          maximumFractionDigits: 6,
                        })}{" "}
                        <span style={{ fontSize: 13 }}>
                          {c.currency_code || "币种未记录"}
                        </span>
                      </div>
                    ))
                : "—"
            }
          >
            {r && !r.costs.some((c) => c.total_cost !== null)
              ? "暂无已记录费用"
              : r?.unbilled_count
                ? `${r.unbilled_count} 次请求未计费`
                : "所选范围累计"}
          </SummaryMetric>
        </div>
      </Spin>
    </section>
  );
}
