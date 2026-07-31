export class ApiClientError extends Error {
  status: number;
  code: string | null;
  body: unknown;

  constructor({
    status,
    message,
    code = null,
    body = null
  }: {
    status: number;
    message: string;
    code?: string | null;
    body?: unknown;
  }) {
    super(message);
    this.name = 'ApiClientError';
    this.status = status;
    this.code = code;
    this.body = body;
  }

  static async fromResponse(response: Response): Promise<ApiClientError> {
    let body: unknown = null;
    let message = `request failed: ${response.status}`;
    let code: string | null = null;
    const contentType = response.headers.get('content-type') ?? '';

    try {
      if (contentType.includes('application/json')) {
        body = (await response.json()) as Record<string, unknown>;
        if (body && typeof body === 'object') {
          if ('message' in body && typeof body.message === 'string') {
            message = body.message;
          }
          if ('code' in body && typeof body.code === 'string') {
            code = body.code;
          }
        }
      } else {
        const text = await response.text();
        if (text) {
          body = text;
        }
      }
    } catch {
      body = null;
    }

    return new ApiClientError({
      status: response.status,
      message,
      code,
      body
    });
  }
}
