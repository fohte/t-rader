import {
  isNoteReferenceDate,
  isNoteReferenceField,
  type NoteReferenceField,
} from '#lib/note-reference'

export type NotePriceReferenceField = NoteReferenceField
export { isNoteReferenceField as isNotePriceReferenceField } from '#lib/note-reference'

export interface ParsedNotePriceReference extends Record<string, string> {
  instrumentId: string
  date: string
  field: NotePriceReferenceField
}

const PRICE_REFERENCE_RE =
  /^\[\[price:([^@\s]+)@(\d{4}-\d{2}-\d{2}):(open|high|low|close|volume)\]\]$/

export function parseNotePriceReference(
  token: string,
): ParsedNotePriceReference | undefined {
  const match = PRICE_REFERENCE_RE.exec(token)
  const instrumentId = match?.[1]
  const date = match?.[2]
  const field = match?.[3]
  if (
    instrumentId == null ||
    instrumentId.length === 0 ||
    date == null ||
    !isNoteReferenceField(field) ||
    !isNoteReferenceDate(date)
  ) {
    return undefined
  }

  return { instrumentId, date, field }
}
