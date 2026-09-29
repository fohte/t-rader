import { err, ok } from 'neverthrow'
import { describe, expect, it } from 'vitest'

import {
  AgentGraphParseError,
  parseAgentGraph,
} from '#strategy-agent/agent-graph/parse'

// zod の issues 配列は JSON 形式でメッセージ末尾に連結される。中身の文言は
// zod のバージョンに依存するため、その部分だけプレースホルダーに正規化する。
const normalizeZodMessage = (message: string): string =>
  message.replace(/: \[[\s\S]*\]$/, ': <zod-issues>')

describe('parseAgentGraph', () => {
  it.each([
    { name: 'an empty string', yaml: '' },
    { name: 'a whitespace-only string', yaml: '   \n' },
  ])('returns an error for $name', ({ yaml }) => {
    expect(parseAgentGraph(yaml)).toEqual(
      err(new AgentGraphParseError('agent_graph is not configured')),
    )
  })

  it('parses a valid multi-phase config into camelCase phases', () => {
    const yaml = `
phases:
  - key: plan
    label: 調査計画
    model: example-model-plan
    reasoning_effort: high
    prompt: 仮説を立てよ
    output:
      hypotheses:
        type: array
        items:
          title: { type: string }
  - key: investigate
    label: 仮説の調査
    model: example-model-investigate
    for_each: plan.hypotheses
    label_field: title
    max_parallel: 4
    prompt: 割り当てられた仮説を検証せよ
    tools: [query_data, write_note]
`

    expect(parseAgentGraph(yaml)).toEqual(
      ok({
        phases: [
          {
            key: 'plan',
            label: '調査計画',
            model: 'example-model-plan',
            reasoningEffort: 'high',
            prompt: '仮説を立てよ',
            skills: [],
            output: {
              hypotheses: {
                type: 'array',
                items: { title: { type: 'string' } },
              },
            },
          },
          {
            key: 'investigate',
            label: '仮説の調査',
            model: 'example-model-investigate',
            prompt: '割り当てられた仮説を検証せよ',
            forEach: 'plan.hypotheses',
            labelField: 'title',
            maxParallel: 4,
            skills: [],
            tools: ['query_data', 'write_note'],
            output: {},
          },
        ],
      }),
    )
  })

  it('leaves tools undefined when omitted, distinct from an explicit empty list', () => {
    const yaml = `
phases:
  - key: plan
    label: 調査計画
    model: example-model-plan
    prompt: 仮説を立てよ
  - key: synthesize
    label: 統合
    model: example-model-synthesize
    prompt: まとめよ
    tools: []
`

    expect(parseAgentGraph(yaml)).toEqual(
      ok({
        phases: [
          {
            key: 'plan',
            label: '調査計画',
            model: 'example-model-plan',
            prompt: '仮説を立てよ',
            skills: [],
            output: {},
          },
          {
            key: 'synthesize',
            label: '統合',
            model: 'example-model-synthesize',
            prompt: 'まとめよ',
            skills: [],
            tools: [],
            output: {},
          },
        ],
      }),
    )
  })

  it('returns an AgentGraphParseError for invalid YAML', () => {
    const result = parseAgentGraph('phases: [')

    expect(result.isErr()).toBe(true)
    expect(result._unsafeUnwrapErr()).toBeInstanceOf(AgentGraphParseError)
    expect(result._unsafeUnwrapErr().message).toBe(
      'agent_graph is not valid YAML',
    )
  })

  it('returns an AgentGraphParseError when a phase is missing a required field', () => {
    const result = parseAgentGraph(`
phases:
  - key: plan
    label: 調査計画
    model: example-model-plan
`)

    expect(result.isErr()).toBe(true)
    const error = result._unsafeUnwrapErr()
    expect(error).toBeInstanceOf(AgentGraphParseError)
    expect(normalizeZodMessage(error.message)).toBe(
      'agent_graph does not match the expected shape: <zod-issues>',
    )
  })
})
