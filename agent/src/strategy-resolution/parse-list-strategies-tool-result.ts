import { err, ok, Result } from 'neverthrow'

import type { StrategyCandidate } from '#strategy-resolution/resolve-strategy'

class StrategyCandidatesParseError extends Error {
  constructor(message: string, cause?: unknown) {
    super(message, cause === undefined ? undefined : { cause })
    this.name = 'StrategyCandidatesParseError'
  }
}

interface ListStrategiesResponseBody {
  strategies: { strategy_id: string; name: string }[]
}

const isListStrategiesResponseBody = (
  value: unknown,
): value is ListStrategiesResponseBody => {
  if (typeof value !== 'object' || value === null) return false
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- value is an untyped bag; each field is narrowed immediately below via typeof
  const record = value as Record<string, unknown>
  const strategies = record['strategies']
  return (
    Array.isArray(strategies) &&
    strategies.every(
      (s: unknown) =>
        typeof s === 'object' &&
        s !== null &&
        // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- s is an untyped bag; each field is narrowed immediately below via typeof
        typeof (s as Record<string, unknown>)['strategy_id'] === 'string' &&
        // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- s is an untyped bag; each field is narrowed immediately below via typeof
        typeof (s as Record<string, unknown>)['name'] === 'string',
    )
  )
}

const isTextContentBlock = (
  value: unknown,
): value is { type: 'text'; text: string } =>
  typeof value === 'object' &&
  value !== null &&
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- value is an untyped bag; each field is narrowed immediately below via typeof
  (value as Record<string, unknown>)['type'] === 'text' &&
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- value is an untyped bag; each field is narrowed immediately below via typeof
  typeof (value as Record<string, unknown>)['text'] === 'string'

const safeJsonParse = Result.fromThrowable(
  (text: string): unknown => JSON.parse(text),
  (error): StrategyCandidatesParseError =>
    new StrategyCandidatesParseError(
      'list_strategies MCP tool returned invalid JSON',
      error,
    ),
)

export const parseListStrategiesToolResult = (
  content: unknown,
): Result<readonly StrategyCandidate[], Error> => {
  if (!Array.isArray(content)) {
    return err(
      new StrategyCandidatesParseError(
        'list_strategies MCP tool returned no content',
      ),
    )
  }
  const textBlock = content.find(isTextContentBlock)
  if (textBlock === undefined) {
    return err(
      new StrategyCandidatesParseError(
        'list_strategies MCP tool returned no text content',
      ),
    )
  }
  return safeJsonParse(textBlock.text).andThen((parsed) => {
    if (!isListStrategiesResponseBody(parsed)) {
      return err(
        new StrategyCandidatesParseError('malformed list_strategies response'),
      )
    }
    return ok(
      parsed.strategies.map((s) => ({
        strategyId: s.strategy_id,
        name: s.name,
      })),
    )
  })
}
