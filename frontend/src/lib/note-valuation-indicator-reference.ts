import { isNoteReferenceDate } from '#lib/note-reference'

const VALUATION_REFERENCE_FIELDS = [
  'per',
  'fwd_per',
  'pbr',
  'roe',
  'fwd_roe',
  'mkt_cap',
  'eps',
  'fwd_eps',
  'bps',
] as const

export type NoteValuationReferenceField =
  (typeof VALUATION_REFERENCE_FIELDS)[number]

export interface ParsedNoteValuationReference extends Record<string, string> {
  instrumentId: string
  date: string
  field: NoteValuationReferenceField
}

export interface ParsedNoteIndicatorObservationReference extends Record<
  string,
  string
> {
  indicatorId: string
  date: string
}

const VALUATION_REFERENCE_RE =
  /^\[\[valuation:([^@\s]+)@(\d{4}-\d{2}-\d{2}):([a-z_]+)\]\]$/
const INDICATOR_OBSERVATION_REFERENCE_RE =
  /^\[\[indicator:([^@\s]+)@(\d{4}-\d{2}-\d{2})\]\]$/

function isValuationReferenceField(
  value: string | undefined,
): value is NoteValuationReferenceField {
  return (
    value != null && VALUATION_REFERENCE_FIELDS.some((field) => field === value)
  )
}

export function parseNoteValuationReference(
  token: string,
): ParsedNoteValuationReference | undefined {
  const match = VALUATION_REFERENCE_RE.exec(token)
  const instrumentId = match?.[1]
  const date = match?.[2]
  const field = match?.[3]
  if (
    instrumentId == null ||
    date == null ||
    !isNoteReferenceDate(date) ||
    !isValuationReferenceField(field)
  ) {
    return undefined
  }

  return { instrumentId, date, field }
}

export function parseNoteIndicatorObservationReference(
  token: string,
): ParsedNoteIndicatorObservationReference | undefined {
  const match = INDICATOR_OBSERVATION_REFERENCE_RE.exec(token)
  const indicatorId = match?.[1]
  const date = match?.[2]
  if (indicatorId == null || date == null || !isNoteReferenceDate(date)) {
    return undefined
  }

  return { indicatorId, date }
}
