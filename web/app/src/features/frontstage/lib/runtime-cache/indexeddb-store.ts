export class IndexedDbUnavailableError extends Error {
  constructor(message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = 'IndexedDbUnavailableError';
  }
}
