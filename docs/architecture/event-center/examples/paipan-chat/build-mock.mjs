// Generates a native, disabled Event Workflow archive without credentials or live sends.
import { readFile, mkdir, writeFile } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const destination = resolve(
  process.argv[2] || "tmp/test-governance/issue-2821/paipan-chat-mock.json",
);
const parser = await readFile(resolve(here, "parse-command.js"), "utf8");
const instanceId = "01a13bd1-3a32-7fa0-9ecc-835f81fd6f60";
const fields = ["arguments", "input_id", "conversation_id", "instance_id"];
const nodes = [];
const edges = [];
function node(id, type, alias, config = {}, bindings = {}, outputs = []) {
  const previous = nodes.at(-1);
  nodes.push({
    id,
    type,
    alias,
    description: alias,
    configVersion: 1,
    config,
    bindings,
    outputs,
    position: { x: 120 + nodes.length * 300, y: 220 },
    containerId: null,
  });
  if (previous)
    edges.push({
      id: `edge-${previous.id}-${id}`,
      source: previous.id,
      target: id,
      sourceHandle: null,
      targetHandle: null,
      containerId: null,
      points: [],
    });
}
const output = {
  key: "result",
  title: "result",
  selector: ["result"],
  valueType: "object",
};
const selector = (...parts) => ({ kind: "selector", value: parts });
function code(id, alias, source, inputs) {
  node(
    id,
    "code",
    alias,
    { language: "javascript", source },
    {
      named_bindings: {
        kind: "named_bindings",
        value: Object.entries(inputs).map(([name, value]) => ({
          name,
          valueType: name === "arguments" ? "string" : "json",
          value:
            value.kind === "selector"
              ? { kind: "selector", selector: value.value }
              : value,
        })),
      },
    },
    [output],
  );
}
node("node-workflow-start", "workflow_start", "原始命令与 Host 身份", {
  input_fields: fields.map((key) => ({
    key,
    label: key,
    valueType: "string",
    inputType: "text",
    source: "body",
    required: key !== "arguments",
    hidden: false,
  })),
  sync_timeout_ms: 30000,
});
code("node-parse", "解析六个必填参数", parser, {
  arguments: selector("node-workflow-start", "arguments"),
});
code(
  "node-calendar-mock",
  "Mock：地区 → 真太阳时 → 八字／紫微",
  `function main(input) {
  const value = input.parsed;
  if (!value.ok) return value;
  const location = value.location_query;
  const birth = value.birth_input;
  if (birth.civil_date !== '2026-11-11' || birth.civil_time !== '13:09:00' || birth.gender !== 'male' || !['广东', '广东省'].includes(location.province) || !['湛江', '湛江市'].includes(location.city)) {
    return {ok:false,status:'mock_case_unavailable',message:'此 Mock 只提供 2026-11-11 13:09:00 男 中国 广东 湛江市的固定参考结果，其他输入尚未计算。'};
  }
  return {ok:true,status:'mock_calculated',mock:true,birth_input:birth,location:{country:'中国',province:'广东省',city:'湛江市',codes:['44','4408'],longitude:110.36,latitude:21.27},true_solar:{civil_date:'2026-11-11',civil_time:'12:46:28',correction_seconds:-1352},as_of:value.as_of,as_of_time:value.as_of_time,bazi:{year:'丙午',month:'己亥',day:'己丑',hour:'庚午'},ziwei:{fiveElementsClass:'水二局',soul:'武曲',body:'火星'}};
}`,
  { parsed: selector("node-parse", "result") },
);
code(
  "node-report",
  "生成报告或补参提示",
  `function main(input) {
  const v = input.calculation;
  if (!v.ok) return {ok:false,status:v.status,report:'【Mock】' + v.message};
  return {ok:true,status:'mock_report_ready',report:['【Mock 排盘报告：固定参考结果，未调用真实接口】','出生：2026-11-11 13:09:00 男','地点：中国 广东省 湛江市（城市参考坐标 110.36, 21.27）','默认真太阳时：' + v.true_solar.civil_date + ' ' + v.true_solar.civil_time,'八字：' + [v.bazi.year,v.bazi.month,v.bazi.day,v.bazi.hour].join(' '),'紫微：' + v.ziwei.fiveElementsClass + '；命主' + v.ziwei.soul + '；身主' + v.ziwei.body,'运限参考时刻：' + v.as_of + ' ' + v.as_of_time + ' UTC+08:00','这里只展示已取证的摘要，完整大运／十二宫仍需真实计算接口。'].join('\\n')};
}`,
  { calculation: selector("node-calendar-mock", "result") },
);
code(
  "node-send-mock",
  "Mock：校验来源并模拟 Notification",
  `function main(input) {
  if (input.instance_id !== '${instanceId}') return {ok:false,status:'source_mismatch',sent:false,mock:true,message:'来源渠道不匹配，不发送报告'};
  if (!/^[0-9a-f-]{36}$/i.test(input.conversation_id) || !/^[0-9a-f-]{36}$/i.test(input.input_id)) return {ok:false,status:'source_invalid',sent:false,mock:true,message:'缺少有效 Host 会话身份，不发送报告'};
  return {ok:input.report.ok,status:input.report.status,mock:true,sent:false,notification_preview:{instance_id:input.instance_id,conversation_id:input.conversation_id,content:input.report.report},message:'Mock 完成，未实际发送微信'};
}`,
  {
    report: selector("node-report", "result"),
    ...Object.fromEntries(
      fields.slice(1).map((key) => [key, selector("node-workflow-start", key)]),
    ),
  },
);
node(
  "node-workflow-end",
  "workflow_end",
  "返回模拟回执（不会自动回复微信）",
  {},
  {
    result: selector("node-send-mock", "result"),
  },
  [output],
);

const archive = {
  schema_version: "1flowbase.application-archive/v1",
  applications: [
    {
      template_id: "paipan-chat-command-mock-2821",
      release_version: 1,
      exported_from_system_version: "0.6.3",
      exported_at: "2026-11-11T00:00:00Z",
      application: {
        application_type: "workflow",
        workflow_trigger_type: "event",
        name: "排盘聊天事件（Mock，未发送）",
        description:
          "Issue #2821：六参数真实校验，地区／排盘固定 Mock，Notification 仅模拟，未发布且不订阅事件。正式事件创建、鉴权、完整排盘与微信发送仍待接通。",
        icon: null,
        icon_type: null,
        icon_background: null,
      },
      flow_document: {
        schemaVersion: "1flowbase.flow/v2",
        meta: {
          flowId: "00000000-0000-4000-8000-000000002821",
          name: "排盘聊天事件 Mock",
          description: "",
          tags: [],
        },
        graph: { nodes, edges },
        editor: {
          viewport: { x: 0, y: 0, zoom: 0.7 },
          annotations: [],
          activeContainerPath: [],
        },
      },
      dependencies: nodes.map((n) => ({
        kind: "builtin_node",
        node_id: n.id,
        node_type: n.type,
        config_version: 1,
      })),
      workflow_trigger_config: null,
    },
  ],
};
await mkdir(dirname(destination), { recursive: true });
await writeFile(destination, JSON.stringify(archive, null, 2) + "\n");
console.log(destination);
