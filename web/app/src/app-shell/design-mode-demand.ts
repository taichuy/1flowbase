import { prepareFrontstageStudio } from '../features/frontstage/lib/jsx-studio/studio-demand';

let topbarNavigationDesignerFlight:
  | Promise<typeof import('./TopbarNavigationDesigner')>
  | undefined;

function loadTopbarNavigationDesigner() {
  if (!topbarNavigationDesignerFlight) {
    const attempt = import('./TopbarNavigationDesigner');
    topbarNavigationDesignerFlight = attempt;
    void attempt.catch(() => {
      if (topbarNavigationDesignerFlight === attempt) {
        topbarNavigationDesignerFlight = undefined;
      }
    });
  }
  return topbarNavigationDesignerFlight;
}

function preloadDesignModeDemand() {
  return Promise.all([
    loadTopbarNavigationDesigner(),
    prepareFrontstageStudio()
  ]).then(
    () => undefined,
    () => undefined
  );
}

export { loadTopbarNavigationDesigner, preloadDesignModeDemand };
