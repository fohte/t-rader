import {
  isNotePriceReferenceField,
  type NotePriceReferenceField,
} from '#lib/note-price-reference'
import { isNoteReferenceDate } from '#lib/note-reference'

export interface ParsedStockChangeReference extends Record<string, string> {
  kind: 'stock'
  instrumentId: string
  start: string
  end: string
  field: NotePriceReferenceField
}

export interface ParsedIndicatorChangeReference extends Record<string, string> {
  kind: 'indicator'
  indicatorId: string
  start: string
  end: string
}

export type ParsedNoteChangeReference =
  ParsedStockChangeReference | ParsedIndicatorChangeReference

const STOCK_CHANGE_REFERENCE_RE =
  /^\[\[change:([^@\s]+)@(\d{4}-\d{2}-\d{2})\.\.(\d{4}-\d{2}-\d{2}):(open|high|low|close|volume)\]\]$/
const INDICATOR_CHANGE_REFERENCE_RE =
  /^\[\[change:indicator:([^@\s]+)@(\d{4}-\d{2}-\d{2})\.\.(\d{4}-\d{2}-\d{2})\]\]$/

function isValidPeriod(start: string, end: string): boolean {
  return isNoteReferenceDate(start) && isNoteReferenceDate(end) && start < end
}

export function parseNoteChangeReference(
  token: string,
): ParsedNoteChangeReference | undefined {
  const isIndicatorToken = token.startsWith('[[change:indicator:')
  const indicatorMatch = INDICATOR_CHANGE_REFERENCE_RE.exec(token)
  const indicatorId = indicatorMatch?.[1]
  const indicatorStart = indicatorMatch?.[2]
  const indicatorEnd = indicatorMatch?.[3]
  if (
    indicatorId != null &&
    indicatorId.length > 0 &&
    indicatorStart != null &&
    indicatorEnd != null &&
    isValidPeriod(indicatorStart, indicatorEnd)
  ) {
    return {
      kind: 'indicator',
      indicatorId,
      start: indicatorStart,
      end: indicatorEnd,
    }
  }
  if (isIndicatorToken) return undefined

  const match = STOCK_CHANGE_REFERENCE_RE.exec(token)
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
    !isValidPeriod(start, end)
  ) {
    return undefined
  }
  return { kind: 'stock', instrumentId, start, end, field }
}
