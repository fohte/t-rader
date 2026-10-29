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
execution_limits:
  max_model_calls: 21
  max_tool_calls: 34
  max_subagent_runs: 8
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
        executionLimits: {
          maxModelCalls: 21,
          maxToolCalls: 34,
          maxSubagentRuns: 8,
        },
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

  it('parses operation phases and all supported input bindings without a model', () => {
    expect(
      parseAgentGraph(`
phases:
  - key: earlier
    label: Earlier
    kind: operation
    operation: evaluate_saved_code
    code_id: demo-source
  - key: prepare
    label: 架空データを変換
    kind: operation
    operation: evaluate_saved_code
    code_id: demo-transform
    input:
      literal_value:
        literal: { count: 3 }
      prior_value:
        prior_output:
          phase: earlier
          pointer: /value/items
          cursor: 4
          limit: 8
      dataset_value:
        dataset:
          provider: demo-source
          params: { collection: demo-set }
    output:
      items: { type: array }
`),
    ).toEqual(
      ok({
        phases: [
          {
            key: 'earlier',
            label: 'Earlier',
            kind: 'operation',
            operation: 'evaluate_saved_code',
            codeId: 'demo-source',
            skills: [],
            subagents: [],
            output: {},
          },
          {
            key: 'prepare',
            label: '架空データを変換',
            kind: 'operation',
            operation: 'evaluate_saved_code',
            codeId: 'demo-transform',
            skills: [],
            subagents: [],
            input: {
              literal_value: { literal: { count: 3 } },
              prior_value: {
                priorOutput: {
                  phase: 'earlier',
                  pointer: '/value/items',
                  cursor: 4,
                  limit: 8,
                },
              },
              dataset_value: {
                dataset: {
                  provider: 'demo-source',
                  params: { collection: 'demo-set' },
                },
              },
            },
            output: { items: { type: 'array' } },
          },
        ],
      }),
    )
  })

  it.each([
    { name: 'for_each', field: 'for_each: prepare.items' },
    { name: 'reasoning_effort', field: 'reasoning_effort: medium' },
    { name: 'tools', field: 'tools: []' },
    { name: 'skills', field: 'skills: []' },
    { name: 'max_subagent_runs', field: 'max_subagent_runs: 1' },
    { name: 'max_parallel', field: 'max_parallel: 2' },
  ])('rejects operation metadata $name', ({ field }) => {
    const result = parseAgentGraph(`
phases:
  - key: prepare
    label: Prepare
    kind: operation
    operation: evaluate_saved_code
    code_id: demo-transform
    ${field}
`)

    expect(
      result.mapErr(
        (error) => new AgentGraphParseError(normalizeZodMessage(error.message)),
      ),
    ).toEqual(
      err(
        new AgentGraphParseError(
          'agent_graph does not match the expected shape: <zod-issues>',
        ),
      ),
    )
  })

  it.each([
    {
      name: 'a later operation phase',
      source: 'later',
      phases: `
  - key: current
    label: Current
    kind: operation
    operation: evaluate_saved_code
    code_id: demo-current
    input:
      rows:
        prior_output: { phase: later, pointer: /value/rows }
  - key: later
    label: Later
    kind: operation
    operation: evaluate_saved_code
    code_id: demo-later`,
    },
    {
      name: 'an LLM phase',
      source: 'earlier',
      phases: `
  - key: earlier
    label: Earlier
    model: demo-model
    prompt: Process this synthetic value.
  - key: current
    label: Current
    kind: operation
    operation: evaluate_saved_code
    code_id: demo-current
    input:
      rows:
        prior_output: { phase: earlier, pointer: /value/rows }`,
    },
  ])('rejects prior_output bound to $name', ({ phases }) => {
    const result = parseAgentGraph(`phases:${phases}`)

    expect(
      result.mapErr(
        (error) => new AgentGraphParseError(normalizeZodMessage(error.message)),
      ),
    ).toEqual(
      err(
        new AgentGraphParseError(
          'agent_graph does not match the expected shape: <zod-issues>',
        ),
      ),
    )
  })

  it('rejects unknown prior_output fields', () => {
    const result = parseAgentGraph(`
phases:
  - key: earlier
    label: Earlier
    kind: operation
    operation: evaluate_saved_code
    code_id: demo-source
  - key: current
    label: Current
    kind: operation
    operation: evaluate_saved_code
    code_id: demo-current
    input:
      rows:
        prior_output:
          phase: earlier
          pointer: /value/rows
          source: unexpected
`)

    expect(
      result.mapErr(
        (error) => new AgentGraphParseError(normalizeZodMessage(error.message)),
      ),
    ).toEqual(
      err(
        new AgentGraphParseError(
          'agent_graph does not match the expected shape: <zod-issues>',
        ),
      ),
    )
  })

  it('rejects a dataset provider containing only whitespace', () => {
    const result = parseAgentGraph(`
phases:
  - key: prepare
    label: Prepare
    kind: operation
    operation: evaluate_saved_code
    code_id: demo-transform
    input:
      rows:
        dataset:
          provider: "   "
          params: {}
`)

    expect(
      result.mapErr(
        (error) => new AgentGraphParseError(normalizeZodMessage(error.message)),
      ),
    ).toEqual(
      err(
        new AgentGraphParseError(
          'agent_graph does not match the expected shape: <zod-issues>',
        ),
      ),
    )
  })

  it('leaves task execution limits unconfigured for legacy YAML', () => {
    expect(parseAgentGraph('phases: []')).toEqual(ok({ phases: [] }))
  })

  it('rejects unknown fields in task execution limits', () => {
    const result = parseAgentGraph(`execution_limits:
  max_model_calls: 2
  unknown_limit: 3
phases: []`).mapErr(
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
    { name: 'tools are omitted', tools: undefined, toolsYaml: '' },
    {
      name: 'the required tool is configured',
      tools: ['demo_tool_a'],
      toolsYaml: '    tools: [demo_tool_a]\n',
    },
  ])('parses required tools when $name', ({ tools, toolsYaml }) => {
    const yaml = `phases:
  - key: stage_a
    label: Stage A
    model: demo-model-a
    prompt: Process the input
${toolsYaml}    required_tools: [demo_tool_a]
`

    expect(parseAgentGraph(yaml)).toEqual(
      ok({
        phases: [
          {
            key: 'stage_a',
            label: 'Stage A',
            model: 'demo-model-a',
            prompt: 'Process the input',
            skills: [],
            subagents: [],
            ...(tools !== undefined ? { tools } : {}),
            requiredTools: ['demo_tool_a'],
            output: {},
          },
        ],
      }),
    )
  })

  it.each([
    {
      name: 'the required tool is not listed',
      toolsYaml: '    tools: [demo_tool_b]\n',
    },
    { name: 'the tool list is empty', toolsYaml: '    tools: []\n' },
  ])('rejects required tools when $name', ({ toolsYaml }) => {
    const normalized = parseAgentGraph(`phases:
  - key: stage_a
    label: Stage A
    model: demo-model-a
    prompt: Process the input
${toolsYaml}    required_tools: [demo_tool_a]
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
