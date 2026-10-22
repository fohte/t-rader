import type { IndicatorObservation } from '#lib/indicator-observation'

export interface NoteIndicatorChangeEvidence {
  startValue: number
  endValue: number
  changePercent: number
}

export type NoteIndicatorChangeEvidenceState =
  | { status: 'loading' }
  | { status: 'error' }
  | { status: 'missing'; missingDates: string[] }
  | { status: 'zero-start'; startValue: number; endValue: number }
  | { status: 'ready'; evidence: NoteIndicatorChangeEvidence }

export function resolveNoteIndicatorChangeEvidence(
  startObservations: IndicatorObservation[],
  endObservations: IndicatorObservation[],
  start: string,
  end: string,
): Exclude<NoteIndicatorChangeEvidenceState, { status: 'loading' | 'error' }> {
  const startValue = startObservations.find(
    (observation) => observation.date === start,
  )?.value
  const endValue = endObservations.find(
    (observation) => observation.date === end,
  )?.value
  const missingDates = [
    ...(startValue == null || !Number.isFinite(startValue) ? [start] : []),
    ...(endValue == null || !Number.isFinite(endValue) ? [end] : []),
  ]

  if (
    startValue == null ||
    endValue == null ||
    !Number.isFinite(startValue) ||
    !Number.isFinite(endValue)
  ) {
    return { status: 'missing', missingDates }
  }

  if (startValue === 0) {
    return { status: 'zero-start', startValue, endValue }
  }

  return {
    status: 'ready',
    evidence: {
      startValue,
      endValue,
      changePercent: ((endValue - startValue) / startValue) * 100,
    },
  }
}
