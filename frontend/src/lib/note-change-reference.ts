import {
  isNotePriceReferenceField,
  type NotePriceReferenceField,
} from '#lib/note-price-reference'
import { isNoteReferenceDate } from '#lib/note-reference'

export interface ParsedNoteChangeReference extends Record<string, string> {
  instrumentId: string
  start: string
  end: string
  field: NotePriceReferenceField
}

const CHANGE_REFERENCE_RE =
  /^\[\[change:([^@\s]+)@(\d{4}-\d{2}-\d{2})\.\.(\d{4}-\d{2}-\d{2}):(open|high|low|close|volume)\]\]$/

export function parseNoteChangeReference(
  token: string,
): ParsedNoteChangeReference | undefined {
  const match = CHANGE_REFERENCE_RE.exec(token)
  const instrumentId = match?.[1]
  const start = match?.[2]
  const end = match?.[3]
  const field = match?.[4]
  if (
    instrumentId == null ||
    instrumentId.length === 0 ||
    start == null ||
    end == null ||
    !isNotePriceReferenceField(field) ||
    !isNoteReferenceDate(start) ||
    !isNoteReferenceDate(end) ||
    start >= end
  ) {
    return undefined
  }
  return { instrumentId, start, end, field }
}
