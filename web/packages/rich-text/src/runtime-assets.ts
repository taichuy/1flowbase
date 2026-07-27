import 'vditor/dist/js/lute/lute.min.js';
import 'vditor/dist/js/i18n/zh_CN.js';
import 'vditor/dist/js/icons/ant.js';

const VDITOR_RUNTIME_MARKERS = [
  'vditorLuteScript',
  'vditorIconScript'
] as const;

export function markBundledVditorRuntime() {
  for (const id of VDITOR_RUNTIME_MARKERS) {
    if (document.getElementById(id)) continue;
    const marker = document.createElement('script');
    marker.id = id;
    marker.type = 'application/x-1flowbase-bundled-support';
    document.head.appendChild(marker);
  }
}
