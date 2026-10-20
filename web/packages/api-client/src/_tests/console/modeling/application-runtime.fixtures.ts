import { afterEach, vi } from 'vitest';

function sseResponse(frame: string) {
  return new Response(
    new ReadableStream({
      start(controller) {
        controller.enqueue(new TextEncoder().encode(frame));
        controller.close();
      }
    }),
    {
      status: 200,
      headers: { 'content-type': 'text/event-stream' }
    }
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

export { sseResponse };
