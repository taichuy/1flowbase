import {
  compileTailwindBase,
  compileTailwindUtilities
} from '@1flowbase/tailwindcss-catalog/compiler';

self.onmessage = async (
  event: MessageEvent<{ requestId: string; candidates: string[] }>
) => {
  const { requestId, candidates } = event.data;
  try {
    const [baseCss, utilities] = await Promise.all([
      compileTailwindBase(),
      compileTailwindUtilities(candidates)
    ]);
    self.postMessage({
      requestId,
      ok: true,
      baseCss,
      utilityCss: utilities.css,
      acceptedCandidates: utilities.acceptedCandidates
    });
  } catch (error) {
    self.postMessage({
      requestId,
      ok: false,
      message:
        error instanceof Error ? error.message : 'Tailwind compilation failed.'
    });
  }
};
