export interface ParsedNoteChangeReference extends Record<string, string> {
  instrumentId: string
  start: string
  end: string
}

const CHANGE_REFERENCE_RE =
  /^\[\[change:([^@\s]+)@(\d{4}-\d{2}-\d{2})\.\.(\d{4}-\d{2}-\d{2}):(open|high|low|close|volume)\]\]$/

function isDate(value: string): boolean {
  const timestamp = Date.parse(`${value}T00:00:00.000Z`)
  return (
    Number.isFinite(timestamp) &&
    new Date(timestamp).toISOString().slice(0, 10) === value
  )
}

export function parseNoteChangeReference(
  token: string,
): ParsedNoteChangeReference | undefined {
  const match = CHANGE_REFERENCE_RE.exec(token)
  const instrumentId = match?.[1]
  const start = match?.[2]
  const end = match?.[3]
  if (
    instrumentId == null ||
    instrumentId.length === 0 ||
    start == null ||
    end == null ||
    !isDate(start) ||
    !isDate(end) ||
    start >= end
  ) {
    return undefined
  }
  return { instrumentId, start, end }
}
