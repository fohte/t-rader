import type { components } from '#lib/api/schema.gen'
import { getChartBarDate } from '#lib/chart-utils'
import type { ParsedNoteChangeReference } from '#lib/note-change-reference'

type Bar = components['schemas']['Bar']

export interface NoteChangeEvidence {
  startValue: number
  endValue: number
  changePercent: number
  matchesSavedValue: boolean
}

export function formatNoteEvidencePrice(value: number): string {
  return new Intl.NumberFormat('ja-JP', {
    maximumSignificantDigits: 15,
  }).format(value)
}

export function formatNoteEvidencePercentage(value: number): string {
  return new Intl.NumberFormat('ja-JP', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
    signDisplay: 'always',
  }).format(value)
}

export function calculateNoteChangeEvidence(
  startValue: number,
  endValue: number,
  savedValue: number,
): NoteChangeEvidence | undefined {
  if (
    !Number.isFinite(startValue) ||
    !Number.isFinite(endValue) ||
    !Number.isFinite(savedValue) ||
    startValue === 0
  ) {
    return undefined
  }

  const changePercent = ((endValue - startValue) / startValue) * 100
  return {
    startValue,
    endValue,
    changePercent,
    matchesSavedValue:
      formatNoteEvidencePercentage(changePercent) ===
      formatNoteEvidencePercentage(savedValue),
  }
}

export function resolveNoteChangeEvidence(
  bars: Bar[],
  reference: ParsedNoteChangeReference,
  savedValue: number,
): NoteChangeEvidence | undefined {
  const startBar = bars.find(
    (bar) => getChartBarDate(bar.timestamp) === reference.start,
  )
  const endBar = bars.find(
    (bar) => getChartBarDate(bar.timestamp) === reference.end,
  )
  const startValue = startBar?.[reference.field]
  const endValue = endBar?.[reference.field]
  if (startValue == null || endValue == null) return undefined

  return calculateNoteChangeEvidence(startValue, endValue, savedValue)
}
