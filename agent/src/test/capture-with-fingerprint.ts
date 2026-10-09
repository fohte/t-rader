export type CaptureWithFingerprintMock = (
  error: unknown,
  fingerprint: string | readonly string[],
  context?: {
    readonly level?: string
    readonly extras?: Readonly<Record<string, unknown>>
  },
) => void

export const normalizeCaptureWithFingerprintCalls = (
  calls: readonly Parameters<CaptureWithFingerprintMock>[],
) =>
  calls.map(([error, fingerprint, context]) => ({
    errorName: error instanceof Error ? error.name : typeof error,
    errorMessage: error instanceof Error ? error.message : String(error),
    fingerprint:
      typeof fingerprint === 'string' ? [fingerprint] : [...fingerprint],
    level: context?.level ?? null,
    extras: context?.extras ?? null,
  }))
