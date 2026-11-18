// Workflow projection of the calendar plugin's generateBaziAIDescription text.
// Inputs are actual calendar DTOs; no chart calculation is duplicated here.
function main(input) {
  const v = input.state;
  const b = v.bazi;
  const chineseDigits = '〇一二三四五六七八九';
  const yearText = year => String(year).split('').map(n => chineseDigits[Number(n)]).join('');
  const stems = '甲乙丙丁戊己庚辛壬癸';
  const branches = '子丑寅卯辰巳午未申酉戌亥';
  const cycle = Array.from({length: 60}, (_, i) => stems[i % 10] + branches[i % 12]);
  const sounds = ['海中金','炉中火','大林木','路旁土','剑锋金','山头火','涧下水','城头土','白蜡金','杨柳木','泉中水','屋上土','霹雳火','松柏木','长流水','沙中金','山下火','平地木','壁上土','金箔金','覆灯火','天河水','大驿土','钗钏金','桑柘木','大溪水','沙中土','天上火','石榴木','大海水'];
  const lunar = v.civil_calendar.lunar;
  const date = v.birth.civil_date.split('-');
  let text = '【八字命盘客观数据】\n\n基本信息：\n';
  text += '- 当前时间：' + date[0] + '年' + date[1] + '月' + date[2] + '日 ' + v.birth.civil_time + '\n';
  text += '- 当前农历：' + yearText(lunar.year) + '年 ' + (lunar.is_leap_month ? '闰' : '') + lunar.month_text + '月 ' + lunar.day_text + '\n';
  text += '- 出生地点：' + v.birth.location.labels.join(' ') + '\n';
  text += '- 经纬度：' + v.birth.location.longitude.toFixed(2) + '°E, ' + v.birth.location.latitude.toFixed(2) + '°N\n';
  text += '- 真太阳时：已启用\n';
  text += '- 真太阳时时间：' + v.true_solar.civil_date + ' ' + v.true_solar.civil_time + '\n';
  text += '- 阳历：' + b.solarDate + '\n- 农历：' + b.lunarDate + '\n- 性别：' + b.gender + '\n';
  text += '- 年龄：' + b.age + '岁 (虚岁' + b.chineseAge + ')\n';
  for (const entry of [['日主','dayMaster'],['格局','geJu'],['胎元','taiYuan'],['命宫','mingGong'],['身宫','shenGong'],['空亡','xunKong']]) text += '- ' + entry[0] + '：' + b[entry[1]] + '\n';
  text += '- 黄道：' + b.huangDao.ecliptic + ' (' + b.huangDao.duty + '日 / ' + b.huangDao.twelveStar + ')\n';
  text += '- 神煞：' + b.shenSha.join('、') + '\n';
  const referenceYear = Number(v.as_of.slice(0,4));
  const referenceGanZhi = cycle[((referenceYear - 4) % 60 + 60) % 60];
  text += '- 当前流年：农历' + referenceGanZhi + '年 (' + referenceGanZhi + ')\n\n';
  text += '八字排盘：\n';
  for (const pair of [['年柱','year'],['月柱','month'],['日柱','day'],['时柱','hour']]) {
    const pillar = b.eightChar[pair[1]];
    const index = cycle.indexOf(pillar.gan_zhi);
    if (index < 0) throw new Error('Invalid calendar pillar');
    const hidden = b.hiddenStems[pillar.earthly_branch] || [];
    text += pair[0] + '：' + pillar.gan_zhi + ' (' + sounds[Math.floor(index / 2)] + ') [' + b.tenGods[pillar.heavenly_stem] + ']\n';
    text += '  藏干：' + hidden.map(h => h + '(' + (b.tenGods[h] || '') + ')').join(' ') + '\n';
  }
  text += '\n五行统计（详细过程）：\n';
  for (const wx of Object.keys(b.wuxingDetail)) {
    const detail = b.wuxingDetail[wx];
    text += '- ' + wx + ' (共' + detail.count + '个)：' + detail.sources.join('，') + '\n';
  }
  text += '\n大运：\n';
  for (const dy of b.daYunDetails) {
    text += dy.startAge + '-' + dy.endAge + '岁：' + dy.name + '运\n';
    text += '  流年：' + dy.years.map(y => y.year + '(' + y.ganZhi + ') ').join('') + '\n';
  }
  text += '\n请基于以上八字数据，进行详细的命理分析，包括喜用神、忌神分析，格局判断（已计算为' + b.geJu + '，请复核），以及大势流年运势预测（结合藏干、十神、空亡、神煞等信息）。';
  return {ok:true, report:text};
}
