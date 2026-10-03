export default function UserUsage({ ctx }) {
  const state = useReport(ctx, false);
  const r = state.report;
  const columns = [
    {
      title: "用户",
      dataIndex: "user_account",
      fixed: "left",
      width: 150,
      render: (value, row) => value || row.user_id || "未归属用户",
    },
    { title: "请求次数", dataIndex: "request_count", align: "right" },
    ...[
      ["总 Tokens", "total_tokens"],
      ["输入 Tokens", "input_tokens"],
      ["输出 Tokens", "output_tokens"],
      ["命中缓存", "input_cache_hit_tokens"],
      ["缓存写入", "cache_write_tokens"],
    ].map(([title, dataIndex]) => ({
      title,
      dataIndex,
      align: "right",
      render: count,
    })),
    {
      title: "累计费用",
      dataIndex: "costs",
      width: 180,
      render: (costs) => <Costs costs={costs} />,
    },
    { title: "未计费请求", dataIndex: "unbilled_count", align: "right" },
  ];
  return (
    <Section title="用户 Token 消耗和费用" state={state}>
      {r && (
        <Table
          columns={columns}
          dataSource={r.users}
          rowKey={(row) => row.user_id || "unattributed"}
          pagination={{
            pageSize: 10,
            hideOnSinglePage: true,
            showSizeChanger: false,
          }}
          scroll={{ x: 1100 }}
          size="middle"
          locale={{ emptyText: "这段时间没有模型请求" }}
        />
      )}
    </Section>
  );
}
