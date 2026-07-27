import 'vditor/dist/js/lute/lute.min.js';
import 'vditor/dist/js/i18n/zh_CN.js';
import 'vditor/dist/js/icons/ant.js';

const VDITOR_RUNTIME_MARKERS = [
  'vditorLuteScript',
  'vditorIconScript'
] as const;

let runtimeConsumers = 0;

export function acquireBundledVditorRuntime(): () => void {
  runtimeConsumers += 1;
  for (const id of VDITOR_RUNTIME_MARKERS) {
    if (document.getElementById(id)) continue;
    const marker = document.createElement('script');
    marker.id = id;
    marker.type = 'application/x-1flowbase-bundled-support';
    document.head.appendChild(marker);
  }
  let released = false;
  return () => {
    if (released) return;
    released = true;
    runtimeConsumers = Math.max(0, runtimeConsumers - 1);
    if (runtimeConsumers === 0) {
      for (const id of VDITOR_RUNTIME_MARKERS) {
        document.getElementById(id)?.remove();
      }
    }
  };
}
