export type NoteReferenceField = 'open' | 'high' | 'low' | 'close' | 'volume'

export function isNoteReferenceField(
  value: string | undefined,
): value is NoteReferenceField {
  return (
    value === 'open' ||
    value === 'high' ||
    value === 'low' ||
    value === 'close' ||
    value === 'volume'
  )
}

export function isNoteReferenceDate(value: string): boolean {
  const timestamp = Date.parse(`${value}T00:00:00.000Z`)
  return (
    Number.isFinite(timestamp) &&
    new Date(timestamp).toISOString().slice(0, 10) === value
  )
}
