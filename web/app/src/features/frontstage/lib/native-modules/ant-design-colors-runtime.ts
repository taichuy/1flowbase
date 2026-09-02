export const ANT_DESIGN_COLORS_EXPORTS = [
  'blue',
  'blueDark',
  'cyan',
  'cyanDark',
  'geekblue',
  'geekblueDark',
  'generate',
  'gold',
  'goldDark',
  'gray',
  'green',
  'greenDark',
  'grey',
  'greyDark',
  'lime',
  'limeDark',
  'magenta',
  'magentaDark',
  'orange',
  'orangeDark',
  'presetDarkPalettes',
  'presetPalettes',
  'presetPrimaryColors',
  'purple',
  'purpleDark',
  'red',
  'redDark',
  'volcano',
  'volcanoDark',
  'yellow',
  'yellowDark'
] as const;

let moduleFlight: Promise<typeof import('@ant-design/colors')> | undefined;

export function loadAntDesignColorsModule(): Promise<
  typeof import('@ant-design/colors')
> {
  moduleFlight ??= import('@ant-design/colors').catch((error: unknown) => {
    moduleFlight = undefined;
    throw error;
  });
  return moduleFlight;
}
