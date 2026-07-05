export function isForbiddenResponseError(error: unknown): boolean {
  return (
    error !== null &&
    typeof error === 'object' &&
    'status' in error &&
    error.status === 403
  );
}
