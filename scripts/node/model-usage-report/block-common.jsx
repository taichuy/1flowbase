import React, { useEffect, useState } from "react";
import {
  Alert,
  Button,
  DatePicker,
  Empty,
  Flex,
  Radio,
  Spin,
  Statistic,
  Table,
  Typography,
  theme,
} from "antd";
import dayjs from "dayjs";
const { Text, Title } = Typography;
const presets = [
  { label: "24小时", value: "24h" },
  { label: "近七天", value: "7d" },
  { label: "近30天", value: "30d" },
  { label: "自定义", value: "custom" },
];
const count = (value) =>
  value === null || value === undefined
    ? "—"
    : Number(value).toLocaleString("zh-CN");
function Costs({ costs }) {
  return (
    <Flex vertical gap={4}>
      {costs
        .filter((c) => c.total_cost !== null)
        .map((c) => (
          <Text key={c.currency_code}>
            {Number(c.total_cost).toLocaleString("zh-CN", {
              maximumFractionDigits: 6,
            })}{" "}
            {c.currency_code || "币种未记录"}
          </Text>
        ))}
      {!costs.some((c) => c.total_cost !== null) && (
        <Text type="secondary">暂无已记录费用</Text>
      )}
    </Flex>
  );
}
function useReport(ctx, initialize) {
  const range = ctx.inputs.timeRange;
  const [report, setReport] = useState(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const publish = async (value) => {
    try {
      const result = await ctx.outputs.publish({ timeRange: value });
      if (result && result.ok === false) setError(true);
    } catch {
      setError(true);
    }
  };
  const select = (preset) => {
    const end = new Date();
    if (preset === "custom") return;
    const days = preset === "24h" ? 1 : preset === "7d" ? 7 : 30;
    publish({
      preset,
      started_from: new Date(end.getTime() - days * 86400000).toISOString(),
      started_to: end.toISOString(),
    });
  };
  useEffect(() => {
    if (initialize && !range) select("24h");
  }, []);
  useEffect(() => {
    if (!range) return;
    let active = true;
    setBusy(true);
    setError(false);
    setReport(null);
    const query =
      "?started_from=" +
      encodeURIComponent(range.started_from) +
      "&started_to=" +
      encodeURIComponent(range.started_to);
    ctx.api
      .get("/api/ex/model-usage-report" + query)
      .then((result) => {
        if (active) setReport(result.report);
      })
      .catch(() => {
        if (active) setError(true);
      })
      .finally(() => {
        if (active) setBusy(false);
      });
    return () => {
      active = false;
    };
  }, [range?.started_from, range?.started_to, refresh]);
  return {
    range,
    report,
    busy,
    error,
    publish,
    select,
    reload: () => setRefresh((n) => n + 1),
  };
}
function Filters({ state, label }) {
  const [custom, setCustom] = useState(false);
  useEffect(() => {
    setCustom(state.range?.preset === "custom");
  }, [state.range]);
  return (
    <Flex wrap gap={10} align="center">
      <Radio.Group
        aria-label={label + "时间范围"}
        optionType="button"
        buttonStyle="solid"
        value={custom ? "custom" : state.range?.preset}
        options={presets}
        onChange={(e) => {
          setCustom(e.target.value === "custom");
          state.select(e.target.value);
        }}
      />
      {custom && (
        <DatePicker.RangePicker
          aria-label={label + "自定义时间"}
          showTime
          allowClear={false}
          value={
            state.range?.preset === "custom"
              ? [dayjs(state.range.started_from), dayjs(state.range.started_to)]
              : null
          }
          onChange={(values) => {
            if (
              values?.[0] &&
              values?.[1] &&
              values[0].valueOf() < values[1].valueOf()
            )
              state.publish({
                preset: "custom",
                started_from: values[0].toISOString(),
                started_to: values[1].toISOString(),
              });
          }}
        />
      )}
      <Button onClick={state.reload} loading={state.busy}>
        刷新
      </Button>
    </Flex>
  );
}
function Section({ title, state, children }) {
  const { token } = theme.useToken();
  return (
    <section
      aria-label={title}
      style={{
        padding: 24,
        background: token.colorBgContainer,
        borderRadius: token.borderRadiusLG,
        border: "1px solid " + token.colorBorderSecondary,
        minWidth: 0,
      }}
    >
      <Flex
        justify="space-between"
        align="center"
        wrap
        gap={16}
        style={{ marginBottom: 24 }}
      >
        <Title level={4} style={{ margin: 0 }}>
          {title}
        </Title>
        <Filters state={state} label={title} />
      </Flex>
      {state.error && (
        <Alert
          type="error"
          showIcon
          title="暂时无法读取报表，请重试"
          style={{ marginBottom: 16 }}
        />
      )}
      <Spin spinning={state.busy}>
        {state.report ? (
          children
        ) : !state.busy && !state.error ? (
          <Empty description="正在初始化时间范围" />
        ) : null}
      </Spin>
      {state.range && (
        <Text
          type="secondary"
          style={{ display: "block", marginTop: 16, fontSize: 12 }}
        >
          统计区间：{dayjs(state.range.started_from).format("YYYY-MM-DD HH:mm")}{" "}
          — {dayjs(state.range.started_to).format("YYYY-MM-DD HH:mm")} ·
          趋势按上海时区分桶
        </Text>
      )}
    </section>
  );
}
