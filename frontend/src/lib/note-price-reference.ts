export type NotePriceReferenceField =
  'open' | 'high' | 'low' | 'close' | 'volume'

export interface ParsedNotePriceReference extends Record<string, string> {
  instrumentId: string
  date: string
  field: NotePriceReferenceField
}

const PRICE_REFERENCE_RE =
  /^\[\[price:([^@\s]+)@(\d{4}-\d{2}-\d{2}):(open|high|low|close|volume)\]\]$/

export function isNotePriceReferenceField(
  value: string | undefined,
): value is NotePriceReferenceField {
  return (
    value === 'open' ||
    value === 'high' ||
    value === 'low' ||
    value === 'close' ||
    value === 'volume'
  )
}

function isDate(value: string): boolean {
  const timestamp = Date.parse(`${value}T00:00:00.000Z`)
  return (
    Number.isFinite(timestamp) &&
    new Date(timestamp).toISOString().slice(0, 10) === value
  )
}

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
    !isNotePriceReferenceField(field) ||
    !isDate(date)
  ) {
    return undefined
  }

  return { instrumentId, date, field }
}
