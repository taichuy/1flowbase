// Workflow projection of the calendar page's generateZiWeiAIDescriptionWithHoroscope.
// Chart calculations and horoscope selection remain in the calendar plugin.
function main(input) {
  const v = input.state;
  const z = v.ziwei;
  const lunar = v.civil_calendar.lunar;
  // The page's age display uses completed solar years and year difference + 1.
  // Project it from this chart's birth/reference dates without requesting a Bazi chart.
  const born = z.birth.civil_date.split('-').map(Number);
  const referenceYear = Number(v.as_of.slice(0, 4));
  const lastDay = new Date(Date.UTC(referenceYear, born[1], 0)).getUTCDate();
  const anniversary = new Date(Date.UTC(referenceYear, born[1] - 1, Math.min(born[2], lastDay)));
  const reference = new Date(v.as_of + 'T12:00:00Z');
  const age = referenceYear - born[0] - (reference < anniversary ? 1 : 0);
  const chineseAge = referenceYear - born[0] + 1;
  const date = v.birth.civil_date.split('-');
  const year = String(lunar.year).split('').map(n => '〇一二三四五六七八九'[Number(n)]).join('');
  let text = '### 【基础信息】\n\n';
  text += '- 当前时间：' + date[0] + '年' + date[1] + '月' + date[2] + '日 ' + v.birth.civil_time + '\n';
  text += '- 当前农历：' + year + '年 ' + (lunar.is_leap_month ? '闰' : '') + lunar.month_text + '月 ' + lunar.day_text + '\n';
  text += '- 出生地点：' + v.birth.location.labels.join(' ') + '\n';
  text += '- 经纬度：' + v.birth.location.longitude.toFixed(2) + '°E, ' + v.birth.location.latitude.toFixed(2) + '°N\n';
  text += '- 真太阳时：已启用\n';
  text += '- 真太阳时时间：' + v.true_solar.civil_date + ' ' + v.true_solar.civil_time + '\n';
  text += '- 阳历生日：' + z.solarDate + '\n- 农历生日：' + z.lunarDate + '\n';
  text += '- 性别：' + (v.birth.gender === 'male' ? '男' : '女') + '\n';
  text += '- 年龄：' + age + '岁 (虚岁' + chineseAge + ')\n';
  text += '- 八字：' + z.chineseDate + '\n- 命主：' + z.soul + '\n- 身主：' + z.body + '\n';
  text += '- 五行局：' + z.fiveElementsClass + '\n\n';
  text += '### 【命盘数据】\n\n';
  for (const p of z.palaces) {
    text += '#### ' + p.name + ' (' + p.heavenlyStem + p.earthlyBranch + ')\n';
    const major = p.majorStars.map(s => s.name + '(' + s.brightness + ')' + (s.mutagen ? '[' + s.mutagen + ']' : '')).join('，');
    text += '- 主星：' + (major || '无') + '\n';
    text += '- 辅星：' + (p.minorStars.map(s => s.name).join('，') || '无') + '\n';
    text += '- 杂曜：' + (p.adjectiveStars.map(s => s.name).join('，') || '无') + '\n';
    text += '- 大限：' + p.decadal.range[0] + '-' + p.decadal.range[1] + '岁\n';
    text += '- 小限：' + p.ages.join('、') + '岁\n\n';
  }
  if (z.horoscope) {
    text += '\n\n### 【运势信息】\n';
    for (const [key, name] of [['decadal', '大限'], ['yearly', '流年'], ['monthly', '流月'], ['daily', '流日']]) {
      const h = z.horoscope[key];
      if (h) text += '- 当前' + name + '：' + h.heavenlyStem + h.earthlyBranch + ' (命宫在' + z.palaces[h.index]?.name + ')\n';
    }
  }
  text += '\n请基于以上客观数据（含本命盘及当前运势），进行详细的紫微斗数运势解读。重点分析当前大限和流年的吉凶祸福，以及需要注意的事项。';
  return { ok: true, report: '【紫微斗数命盘客观数据】\n\n' + text };
}
