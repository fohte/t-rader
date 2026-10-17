export function getApiErrorMessage(error: unknown, fallback: string): string {
  if (error != null && typeof error === 'object') {
    if ('error' in error && typeof error.error === 'string') return error.error
    if (
      'data' in error &&
      error.data != null &&
      typeof error.data === 'object' &&
      'error' in error.data &&
      typeof error.data.error === 'string'
    ) {
      return error.data.error
    }
  }
  return error instanceof Error && error.message !== ''
    ? error.message
    : fallback
}

export function getApiErrorStatus(error: unknown): number | null {
  if (error == null || typeof error !== 'object' || !('status' in error)) {
    return null
  }
  return typeof error.status === 'number' ? error.status : null
}
