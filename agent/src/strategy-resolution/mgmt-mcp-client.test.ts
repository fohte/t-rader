import { err, ok } from 'neverthrow'
import { describe, expect, it } from 'vitest'

import { parseListStrategiesToolResult } from '#strategy-resolution/parse-list-strategies-tool-result'

const errorNameAndMessage = (
  result: ReturnType<typeof parseListStrategiesToolResult>,
) => result.mapErr(({ name, message }) => ({ name, message }))

describe('parseListStrategiesToolResult', () => {
  it('maps a well-formed list_strategies text content to candidates', () => {
    const content = [
      {
        type: 'text',
        text: JSON.stringify({
          strategies: [
            {
              strategy_id: '11111111-1111-1111-1111-111111111111',
              name: '長期投資',
            },
            {
              strategy_id: '22222222-2222-2222-2222-222222222222',
              name: '中期投資',
            },
          ],
        }),
      },
    ]

    expect(parseListStrategiesToolResult(content)).toEqual(
      ok([
        {
          strategyId: '11111111-1111-1111-1111-111111111111',
          name: '長期投資',
        },
        {
          strategyId: '22222222-2222-2222-2222-222222222222',
          name: '中期投資',
        },
      ]),
    )
  })

  it('returns an error when there is no text content block', () => {
    expect(errorNameAndMessage(parseListStrategiesToolResult([]))).toEqual(
      err({
        name: 'StrategyCandidatesParseError',
        message: 'list_strategies MCP tool returned no text content',
      }),
    )
  })

  it('returns an error when the text content is not valid JSON', () => {
    expect(
      errorNameAndMessage(
        parseListStrategiesToolResult([{ type: 'text', text: 'not json' }]),
      ),
    ).toEqual(
      err({
        name: 'StrategyCandidatesParseError',
        message: 'list_strategies MCP tool returned invalid JSON',
      }),
    )
  })

  it('returns an error when the parsed body does not match the expected shape', () => {
    expect(
      errorNameAndMessage(
        parseListStrategiesToolResult([
          {
            type: 'text',
            text: JSON.stringify({ strategies: [{ name: '長期投資' }] }),
          },
        ]),
      ),
    ).toEqual(
      err({
        name: 'StrategyCandidatesParseError',
        message: 'malformed list_strategies response',
      }),
    )
  })
})
