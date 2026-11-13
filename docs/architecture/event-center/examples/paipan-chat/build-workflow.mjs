// Build a native Event Workflow draft. No API credentials are embedded or provisioned.
import { readFile, mkdir, writeFile } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const destination = resolve(
  process.argv[2] || "tmp/test-governance/issue-2821/paipan-chat-workflow.json",
);
const instanceId = process.argv[3];
if (
  !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
    instanceId || "",
  )
)
  throw new Error(
    "Provide the WeChat channel instance UUID as the second argument.",
  );
const chart = process.argv[4] || "bazi";
if (!["bazi", "ziwei"].includes(chart)) throw new Error("Chart must be bazi or ziwei");
const chartName = chart === "bazi" ? "八字" : "紫微";
const commandName = chart === "bazi" ? "paipan" : "ziwei";
const parser = (await readFile(resolve(here, "parse-command.js"), "utf8")).replace("/paipan ", `/${commandName} `);
const formatter = await readFile(resolve(here, `format-${chart}-report.js`), "utf8");
const fields = ["arguments", "input_id", "conversation_id", "instance_id"];
const nodes = [],
  edges = [];
const output = {
  key: "result",
  title: "result",
  selector: ["result"],
  valueType: "object",
};
const selected = (...selector) => ({ kind: "selector", selector });
const binding = (...value) => ({ kind: "selector", value });
const constant = (value) => ({ kind: "constant", value });
function add(id, type, alias, config = {}, bindings = {}, outputs = []) {
  nodes.push({
    id,
    type,
    alias,
    description: alias,
    configVersion: 1,
    config,
    bindings,
    outputs,
    position: {
      x: 120 + Math.floor(nodes.length / 5) * 350,
      y: (nodes.length % 5) * 160,
    },
    containerId: null,
  });
  return id;
}
function edge(source, target, sourceHandle = null) {
  edges.push({
    id: `edge-${source}-${target}`,
    source,
    target,
    sourceHandle,
    targetHandle: null,
    containerId: null,
    points: [],
  });
}
function code(id, alias, source, inputs) {
  return add(
    id,
    "code",
    alias,
    { language: "javascript", source },
    {
      named_bindings: {
        kind: "named_bindings",
        value: Object.entries(inputs).map(([name, value]) => ({
          name,
          valueType: "json",
          value,
        })),
      },
    },
    [output],
  );
}
function end(id, source) {
  add(
    id,
    "workflow_end",
    "结束（不自动回复）",
    {},
    { result: binding(source, "result") },
    [output],
  );
  edge(source, id);
}
function notify(id, source, field, alias = "向指定来源微信渠道发送") {
  add(
    id,
    "notification",
    alias,
    { instance_id: instanceId },
    {
      input: {
        kind: "named_bindings",
        value: [
          {
            name: "text",
            valueType: "string",
            value: selected(source, "result", field),
          },
        ],
      },
    },
    [output],
  );
  return id;
}
function branch(source, allowReply = true) {
  const id = `${source}-branch`;
  add(
    id,
    "if_else",
    "成功继续／失败结束",
    {},
    {
      branches: {
        kind: "if_else_branches",
        value: {
          branches: [
            {
              id: "if",
              kind: "if",
              title: "成功",
              sourceHandle: "if",
              condition: {
                operator: "and",
                conditions: [
                  {
                    kind: "rule",
                    left: [source, "result", "ok"],
                    comparator: "equals",
                    right: constant(true),
                  },
                ],
              },
            },
            { id: "else", kind: "else", title: "失败", sourceHandle: "else" },
          ],
        },
      },
    },
  );
  edge(source, id);
  if (allowReply) {
    const reply = notify(`${source}-error-notification`, source, "message");
    edge(id, reply, "else");
    end(`${source}-error-end`, reply);
  } else {
    const terminal = `${source}-error-end`;
    add(
      terminal,
      "workflow_end",
      "来源不匹配：不发送",
      {},
      { result: binding(source, "result") },
      [output],
    );
    edge(id, terminal, "else");
  }
  return id;
}
function connectSuccess(sourceBranch, target) {
  edge(sourceBranch, target, "if");
}

add("node-workflow-start", "workflow_start", "聊天事件原始输入", {
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
code(
  "node-source",
  "校验渠道来源",
  `function main(input) { return {ok:input.instance_id === ${JSON.stringify(instanceId)},message:'来源渠道不匹配，已停止'}; }`,
  { instance_id: selected("node-workflow-start", "instance_id") },
);
edge("node-workflow-start", "node-source");
const sourceBranch = branch("node-source", false);
code("node-parse", "六个必填参数，默认真太阳时", parser, {
  arguments: selected("node-workflow-start", "arguments"),
});
connectSuccess(sourceBranch, "node-parse");
let previous = branch("node-parse");

// Each HTTP response is decoded once; only the documented transport envelope is removed.
const decode = `
  if (input.status < 200 || input.status >= 300) return {ok:false,message:'日历接口请求失败（HTTP ' + input.status + '），请稍后重试'};
  let envelope;
  try { envelope = JSON.parse(input.body); } catch (_) { return {ok:false,message:'日历接口返回了无效 JSON'}; }
  const result = envelope.data;
  if (!result || result.ok !== true) return {ok:false,message:'日历计算失败：' + (result && result.error ? result.error.message : '接口返回契约不匹配')};
  const data = result.data;
  const state = input.state;
`;
function request(
  id,
  alias,
  operation,
  requestExpression,
  stateNode,
  transform,
) {
  const prepare = `${id}-request`;
  code(
    prepare,
    "构造" + alias + "参数",
    `function main(input) { const state = input.state; return {body:JSON.stringify(${requestExpression})}; }`,
    { state: selected(stateNode, "result") },
  );
  connectSuccess(previous, prepare);
  add(
    id,
    "http_request",
    alias,
    {
      method: "POST",
      url: `{{env.CalendarApiOrigin}}/api/console/managed-services/taichuy-calendar/${operation}`,
      body_type: "json",
      timeout_ms: 30000,
    },
    {
      body: binding(prepare, "result", "body"),
      headers: {
        kind: "named_bindings",
        value: [
          {
            name: "Authorization",
            valueType: "string",
            value: {
              kind: "templated_text",
              value: "{{env.CalendarApiAuthorization}}",
            },
          },
        ],
      },
    },
    [
      { key: "body", title: "body", valueType: "string", selector: ["body"] },
      {
        key: "status_code",
        title: "status_code",
        valueType: "number",
        selector: ["status_code"],
      },
    ],
  );
  edge(prepare, id);
  const checked = `${id}-checked`;
  code(
    checked,
    "核对" + alias + "结果",
    `function main(input) { ${decode}\n${transform}\n}`,
    {
      status: selected(id, "status_code"),
      body: selected(id, "body"),
      state: selected(stateNode, "result"),
    },
  );
  edge(id, checked);
  previous = branch(checked);
  return checked;
}
const province = request(
  "node-provinces",
  "查询省份",
  "regions/children",
  "{parent_codes:[]}",
  "node-parse",
  `
  const normalize = name => name.replace(/(省|市|壮族自治区|回族自治区|维吾尔自治区|自治区|特别行政区)$/, '');
  const matches = data.children.filter(item => normalize(item.name) === normalize(state.location_query.province));
  if (matches.length !== 1) return {ok:false,message:'无法唯一识别省份：' + state.location_query.province};
  return {...state,province:matches[0]};`,
);
const city = request(
  "node-cities",
  "查询城市",
  "regions/children",
  "{parent_codes:[state.province.code]}",
  province,
  `
  const normalize = name => name.replace(/市$/, '');
  const matches = data.children.filter(item => normalize(item.name) === normalize(state.location_query.city));
  if (matches.length !== 1) return {ok:false,message:'无法唯一识别城市：' + state.location_query.city + '，请核对所属省份'};
  return {...state,city:matches[0],location_codes:[state.province.code,matches[0].code]};`,
);
const birth = request(
  "node-birth",
  "规范化出生信息",
  "birth/normalize",
  "{...state.birth_input,location_codes:state.location_codes}",
  city,
  `
  if (!Number.isFinite(data.location.longitude) || !Number.isFinite(data.location.latitude)) return {ok:false,message:'该城市缺少参考坐标，无法计算真太阳时'};
  return {...state,birth:data};`,
);
const birthCalendar = request(
  "node-civil-calendar", "查询出生日期农历", "calendar/convert",
  "{calendar:'solar',year:Number(state.birth.civil_date.slice(0,4)),month:Number(state.birth.civil_date.slice(5,7)),day:Number(state.birth.civil_date.slice(8,10)),is_leap_month:false}",
  birth,
  "return {...state,civil_calendar:data};",
);
const solar = request(
  "node-solar",
  "计算真太阳时",
  "time/true-solar",
  "{civil_date:state.birth.civil_date,civil_time:state.birth.civil_time,utc_offset_minutes:480,longitude:state.birth.location.longitude,latitude:state.birth.location.latitude}",
  birthCalendar,
  `
  return {...state,true_solar:data,chart_input:{civil_date:data.civil_date,civil_time:data.civil_time,utc_offset_minutes:480,gender:state.birth.gender,location_codes:state.location_codes}};`,
);
const chartResult = request(
  `node-${chart}`,
  `计算${chartName}`,
  `${chart}/calculate`,
  chart === "bazi" ? "{...state.chart_input,as_of:state.as_of}" : "{...state.chart_input,as_of:state.as_of,as_of_time:state.as_of_time}",
  solar,
  `return {...state,${chart}:data};`,
);
code("node-report", `整理完整${chartName}报告`, formatter, {
  state: selected(chartResult, "result"),
});
connectSuccess(previous, "node-report");
notify("node-notification", "node-report", "report", `发送完整${chartName}报告`);
edge("node-report", "node-notification");
end("node-workflow-end", "node-notification");

const archive = {
  schema_version: "1flowbase.application-archive/v1",
  applications: [
    {
      template_id: `${commandName}-chat-command-2821`,
      release_version: 1,
      exported_from_system_version: "0.6.3",
      exported_at: "2026-11-11T00:00:00Z",
      application: {
        application_type: "workflow",
        workflow_trigger_type: "event",
        name: `${chartName}报告（聊天指令）`,
        description:
          "未发布。需配置日历接口 origin/鉴权、真实聊天事件及输入映射；运行会调用真实计算接口并通过 Notification 向指定微信渠道发送。",
        icon: null,
        icon_type: null,
        icon_background: null,
      },
      flow_document: {
        schemaVersion: "1flowbase.flow/v2",
        meta: {
          flowId: "00000000-0000-4000-8000-000000002821",
          name: `${chartName}聊天事件`,
          description: "",
          tags: [],
        },
        graph: { nodes, edges },
        editor: {
          viewport: { x: 0, y: 0, zoom: 0.5 },
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
