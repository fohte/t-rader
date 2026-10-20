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
  tool_a: ${value}
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
  tool_a: 4
tool_models:
  tool_a: demo-model-tool-a
  tool_b: demo-model-tool-b
phases:
  - key: stage_a
    label: 項目の作成
    model: demo-model-a
    reasoning_effort: high
    prompt: 架空の項目を作成してください
    output:
      entries:
        type: array
        items:
          label: { type: string }
  - key: stage_b
    label: 項目の処理
    model: demo-model-b
    timeout_minutes: 15
    for_each: stage_a.entries
    label_field: label
    max_parallel: 4
    subagents: [helper_a, helper_b]
    max_subagent_runs: 3
    prompt: 受け取った項目を文章にしてください
    tools: [read_records, write_record]
`

    expect(parseAgentGraph(yaml)).toEqual(
      ok({
        timeoutMinutes: 30,
        toolLimits: { tool_a: 4 },
        toolModels: {
          tool_a: 'demo-model-tool-a',
          tool_b: 'demo-model-tool-b',
        },
        phases: [
          {
            key: 'stage_a',
            label: '項目の作成',
            model: 'demo-model-a',
            reasoningEffort: 'high',
            prompt: '架空の項目を作成してください',
            skills: [],
            subagents: [],
            output: {
              entries: {
                type: 'array',
                items: { label: { type: 'string' } },
              },
            },
          },
          {
            key: 'stage_b',
            label: '項目の処理',
            model: 'demo-model-b',
            timeoutMinutes: 15,
            prompt: '受け取った項目を文章にしてください',
            forEach: 'stage_a.entries',
            labelField: 'label',
            maxParallel: 4,
            skills: [],
            subagents: ['helper_a', 'helper_b'],
            maxSubagentRuns: 3,
            tools: ['read_records', 'write_record'],
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
        : `phases:\n  - key: sample\n    label: Sample\n    model: demo-model-a\n    prompt: sample\n    timeout_minutes: ${value}`
    const yaml =
      target === 'graph'
        ? `${timeoutConfig}\nphases:\n  - key: sample\n    label: Sample\n    model: demo-model-a\n    prompt: sample`
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
  - key: stage_a
    label: 項目の作成
    model: demo-model-a
    prompt: 架空の項目を作成してください
  - key: stage_b
    label: 項目の整理
    model: demo-model-b
    prompt: 項目を整理してください
    tools: []
`

    expect(parseAgentGraph(yaml)).toEqual(
      ok({
        phases: [
          {
            key: 'stage_a',
            label: '項目の作成',
            model: 'demo-model-a',
            prompt: '架空の項目を作成してください',
            skills: [],
            subagents: [],
            output: {},
          },
          {
            key: 'stage_b',
            label: '項目の整理',
            model: 'demo-model-b',
            prompt: '項目を整理してください',
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
  - key: stage_a
    label: 項目の処理
    model: demo-model-a
    prompt: 項目を処理してください
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
  - key: stage_a
    label: 項目の処理
    model: demo-model-a
    prompt: 項目を処理してください
    subagents: [helper_a]
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
  - key: stage_a
    label: 項目の作成
    model: demo-model-a
`)

    expect(result.isErr()).toBe(true)
    const error = result._unsafeUnwrapErr()
    expect(error).toBeInstanceOf(AgentGraphParseError)
    expect(normalizeZodMessage(error.message)).toBe(
      'agent_graph does not match the expected shape: <zod-issues>',
    )
  })
})
