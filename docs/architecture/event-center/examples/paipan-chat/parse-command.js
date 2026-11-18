// Workflow Code node source. Business validation returns data so the workflow can reply.
function main(input) {
  const example =
    "/paipan --brith=2026-11-11 --time=13:09:00 --gender=男 --country=中国 --province=广东 --city=湛江市";
  const fail = (message) => ({
    ok: false,
    status: "invalid",
    message: message + "\n用法：" + example,
  });
  const required = ["brith", "time", "gender", "country", "province", "city"];
  const values = {};
  for (const token of String(input.arguments || "")
    .trim()
    .split(/\s+/)
    .filter(Boolean)) {
    const flag = /^--([a-z_]+)=(.+)$/.exec(token);
    if (!flag || !required.includes(flag[1])) return fail("无效参数：" + token);
    if (Object.prototype.hasOwnProperty.call(values, flag[1]))
      return fail("重复参数：--" + flag[1]);
    values[flag[1]] = flag[2];
  }
  const missing = required.filter((key) => !values[key]);
  if (missing.length)
    return fail("缺少必填参数：" + missing.map((key) => "--" + key).join("、"));
  const date = new Date(values.brith + "T00:00:00Z");
  if (
    !/^\d{4}-\d{2}-\d{2}$/.test(values.brith) ||
    !Number.isFinite(date.getTime()) ||
    date.toISOString().slice(0, 10) !== values.brith
  )
    return fail("出生日期无效，请填写真实日期 YYYY-MM-DD");
  if (!/^(?:[01]\d|2[0-3]):[0-5]\d:[0-5]\d$/.test(values.time))
    return fail("出生时间无效，请填写 HH:mm:ss");
  if (!["男", "女"].includes(values.gender)) return fail("性别必须填写男或女");
  if (values.country !== "中国") return fail("当前只支持中国地址");
  if (
    !/^[\u3400-\u9fff]+$/.test(values.province) ||
    !/^[\u3400-\u9fff]+$/.test(values.city)
  )
    return fail("省和市必须填写汉字名称");
  const now = new Date(Date.now() + 480 * 60000).toISOString();
  return {
    ok: true,
    status: "validated",
    birth_input: {
      civil_date: values.brith,
      civil_time: values.time,
      gender: values.gender === "男" ? "male" : "female",
      utc_offset_minutes: 480,
    },
    location_query: {
      country: values.country,
      province: values.province,
      city: values.city,
    },
    true_solar: true,
    as_of: now.slice(0, 10),
    as_of_time: now.slice(11, 19),
  };
}
