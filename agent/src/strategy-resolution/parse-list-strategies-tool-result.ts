import { err, ok, Result } from 'neverthrow'

import { isPlainObject } from '#strategy-agent/agent-graph/json'
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
  if (!isPlainObject(value)) return false
  const record = value
  const strategies = record['strategies']
  return (
    Array.isArray(strategies) &&
    strategies.every(
      (s: unknown) =>
        isPlainObject(s) &&
        typeof s['strategy_id'] === 'string' &&
        typeof s['name'] === 'string',
    )
  )
}

const isTextContentBlock = (
  value: unknown,
): value is { type: 'text'; text: string } =>
  isPlainObject(value) &&
  value['type'] === 'text' &&
  typeof value['text'] === 'string'

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
