import React, { useEffect, useRef, useState } from "react";
import {
  Alert,
  Button,
  DatePicker,
  Empty,
  Flex,
  Radio,
  Spin,
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
// One authorized producer publishes a retained snapshot to the two views.
// The request belongs to the retained component; an effect only subscribes to it.
function useReport(ctx, owner) {
  const range = ctx.inputs.timeRange;
  const empty = { report: null, busy: false, error: false };
  const [local, setLocal] = useState(() => ctx.inputs.reportState || empty);
  const snapshot = useRef(local);
  const request = useRef(null);
  const published = useRef(null);
  const initialized = useRef(false);
  const [retryRevision, setRetryRevision] = useState(0);
  const state = owner ? local : ctx.inputs.reportState || empty;
  const publish = async (value) => {
    try {
      const result = await ctx.outputs.publish({
        timeRange: value,
        reportState: snapshot.current,
      });
      if (result?.ok === false) throw new Error(result.error);
    } catch {
      setLocal((previous) => ({ ...previous, error: true }));
    }
  };
  const select = (preset) => {
    if (preset === "custom") return;
    const end = new Date();
    const days = preset === "24h" ? 1 : preset === "7d" ? 7 : 30;
    return publish({
      preset,
      started_from: new Date(end.getTime() - days * 86400000).toISOString(),
      started_to: end.toISOString(),
    });
  };
  useEffect(() => {
    if (owner && !range && !initialized.current) {
      initialized.current = true;
      void select("24h");
    }
  }, []);
  useEffect(() => {
    if (!owner || !range) return;
    const key = JSON.stringify([
      range.started_from,
      range.started_to,
      retryRevision,
    ]);
    let entry = request.current;
    if (!entry || entry.key !== key) {
      entry = {
        key,
        snapshot: { ...snapshot.current, busy: true, error: false },
      };
      request.current = entry;
      entry.promise = ctx.api
        .get("/api/ex/model-usage-report", {
          query: {
            started_from: range.started_from,
            started_to: range.started_to,
          },
        })
        .then(
          ({ report }) => {
            entry.snapshot = { report, busy: false, error: false };
          },
          () => {
            entry.snapshot = { ...entry.snapshot, busy: false, error: true };
          },
        );
    }
    let subscribed = true;
    const commit = () => {
      if (!subscribed || request.current !== entry) return;
      snapshot.current = entry.snapshot;
      setLocal(entry.snapshot);
      if (published.current === entry.snapshot) return;
      // Capability publication only occurs while this effect is active. A
      // completion while hidden is retained and published on the next reveal.
      const next = entry.snapshot;
      void Promise.resolve(
        ctx.outputs.publish({ timeRange: range, reportState: next }),
      )
        .then((result) => {
          if (result?.ok === false) throw new Error(result.error);
          published.current = next;
        })
        .catch(() => {
          if (subscribed && request.current === entry)
            setLocal((previous) => ({ ...previous, error: true }));
        });
    };
    commit();
    void entry.promise.then(commit);
    return () => {
      subscribed = false;
    };
  }, [owner, range?.started_from, range?.started_to, retryRevision]);
  return {
    ...state,
    range,
    publish,
    select,
    retry: () => setRetryRevision((n) => n + 1),
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
      </Flex>
      {state.error && (
        <Alert
          type="error"
          showIcon
          title="暂时无法读取报表，请重试"
          style={{ marginBottom: 16 }}
        />
      )}
      <Spin spinning={state.busy} description={state.busy ? "正在更新" : undefined}>
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
          统计区间：
          {dayjs(state.report?.started_from || state.range.started_from).format(
            "YYYY-MM-DD HH:mm",
          )}{" "}
          —{" "}
          {dayjs(state.report?.started_to || state.range.started_to).format(
            "YYYY-MM-DD HH:mm",
          )}{" "}
          · 趋势按上海时区分桶
        </Text>
      )}
    </section>
  );
}
