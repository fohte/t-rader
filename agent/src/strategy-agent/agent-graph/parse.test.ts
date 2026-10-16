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

  it.each([
    { name: 'negative', value: '-1' },
    { name: 'fractional', value: '1.5' },
    { name: 'above the integer range', value: '4294967296' },
  ])('rejects a task tool limit that is $name', ({ value }) => {
    const result = parseAgentGraph(
      `tool_limits:
  search_web: ${value}
phases: []`,
    ).mapErr(
      (error) => new AgentGraphParseError(normalizeZodMessage(error.message)),
    )

    expect(result).toEqual(
      err(
        new AgentGraphParseError(
          'agent_graph does not match the expected shape: <zod-issues>',
        ),
      ),
    )
  })

  it('parses a valid multi-phase config into camelCase phases', () => {
    const yaml = `
timeout_minutes: 30
tool_limits:
  search_web: 4
tool_models:
  search_web: example-model-search
  query_youtube: example-model-youtube
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
    timeout_minutes: 15
    for_each: plan.hypotheses
    label_field: title
    max_parallel: 4
    subagents: [researcher, reviewer]
    max_subagent_runs: 3
    prompt: 割り当てられた仮説を検証せよ
    tools: [query_data, write_note]
`

    expect(parseAgentGraph(yaml)).toEqual(
      ok({
        timeoutMinutes: 30,
        toolLimits: { search_web: 4 },
        toolModels: {
          search_web: 'example-model-search',
          query_youtube: 'example-model-youtube',
        },
        phases: [
          {
            key: 'plan',
            label: '調査計画',
            model: 'example-model-plan',
            reasoningEffort: 'high',
            prompt: '仮説を立てよ',
            skills: [],
            subagents: [],
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
            timeoutMinutes: 15,
            prompt: '割り当てられた仮説を検証せよ',
            forEach: 'plan.hypotheses',
            labelField: 'title',
            maxParallel: 4,
            skills: [],
            subagents: ['researcher', 'reviewer'],
            maxSubagentRuns: 3,
            tools: ['query_data', 'write_note'],
            output: {},
          },
        ],
      }),
    )
  })

  it.each([
    { name: 'zero at the graph level', target: 'graph', value: '0' },
    { name: 'too large at the graph level', target: 'graph', value: '361' },
    { name: 'fractional at the graph level', target: 'graph', value: '1.5' },
    { name: 'zero in a phase', target: 'phase', value: '0' },
    { name: 'too large in a phase', target: 'phase', value: '361' },
    { name: 'fractional in a phase', target: 'phase', value: '1.5' },
  ])('rejects a timeout that is $name', ({ target, value }) => {
    const timeoutConfig =
      target === 'graph'
        ? `timeout_minutes: ${value}`
        : `phases:\n  - key: sample\n    label: Sample\n    model: example-model-sample\n    prompt: sample\n    timeout_minutes: ${value}`
    const yaml =
      target === 'graph'
        ? `${timeoutConfig}\nphases:\n  - key: sample\n    label: Sample\n    model: example-model-sample\n    prompt: sample`
        : timeoutConfig

    const normalized = parseAgentGraph(yaml).mapErr(
      (error) => new AgentGraphParseError(normalizeZodMessage(error.message)),
    )
    expect(normalized).toEqual(
      err(
        new AgentGraphParseError(
          'agent_graph does not match the expected shape: <zod-issues>',
        ),
      ),
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
            subagents: [],
            output: {},
          },
          {
            key: 'synthesize',
            label: '統合',
            model: 'example-model-synthesize',
            prompt: 'まとめよ',
            skills: [],
            subagents: [],
            tools: [],
            output: {},
          },
        ],
      }),
    )
  })

  it.each([
    { name: 'negative', value: '-1' },
    { name: 'fractional', value: '1.5' },
    { name: 'larger than an unsigned 32-bit integer', value: '4294967296' },
  ])('rejects a $name max_subagent_runs value', ({ value }) => {
    const yaml = `
phases:
  - key: research
    label: Research
    model: example-model-research
    prompt: Investigate
    max_subagent_runs: ${value}
`

    const normalized = parseAgentGraph(yaml).mapErr(
      (error) => new AgentGraphParseError(normalizeZodMessage(error.message)),
    )
    expect(normalized).toEqual(
      err(
        new AgentGraphParseError(
          'agent_graph does not match the expected shape: <zod-issues>',
        ),
      ),
    )
  })

  it('rejects subagents when max_subagent_runs is omitted', () => {
    const normalized = parseAgentGraph(`
phases:
  - key: research
    label: Research
    model: example-model-research
    prompt: Investigate
    subagents: [helper]
`).mapErr(
      (error) => new AgentGraphParseError(normalizeZodMessage(error.message)),
    )

    expect(normalized).toEqual(
      err(
        new AgentGraphParseError(
          'agent_graph does not match the expected shape: <zod-issues>',
        ),
      ),
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
