import { BaseChatModel } from '@langchain/core/language_models/chat_models'
import type { ChatResult } from '@langchain/core/outputs'
import type { Result } from 'neverthrow'
import { err, ok } from 'neverthrow'
import { describe, expect, it } from 'vitest'

import { runAgentGraph } from '#strategy-agent/agent-graph/run-agent-graph'
import type {
  RunAgentGraphContext,
  RunAgentGraphDeps,
} from '#strategy-agent/agent-graph/run-agent-graph/types'
import type { StrategyTaskStep } from '#strategy-agent/agent-graph/step'
import type { AgentGraphConfig } from '#strategy-agent/agent-graph/types'
import type { ExecutionToolRequest } from '#strategy-agent/execution-tool-client'
import type { McpToolsClient } from '#strategy-agent/strategy-agent'
import { normalizeStepTimestamps } from '#test/normalize-step-timestamps'

const NOOP_TRACE_ID = '00000000000000000000000000000000'
const NOOP_SPAN_ID = '0000000000000000'

class FakeChatModel extends BaseChatModel {
  override _llmType(): string {
    return 'fake'
  }

  override _generate(): Promise<ChatResult> {
    return Promise.reject(new Error('FakeChatModel should not be invoked'))
  }
}

const payload = (id: string, digest: string): Record<string, unknown> => ({
  id,
  digest,
})

const operationResponse = (
  inputId: string,
  outputId: string,
  outputDigest: string,
): Record<string, unknown> => ({
  input_payload: payload(inputId, 'demo-input-digest'),
  output_payload: payload(outputId, outputDigest),
  exit_code: 0,
  stderr: '',
})

const createDeps = (
  modelCalls: string[],
  invoke: () => Promise<Record<string, unknown>>,
): RunAgentGraphDeps => ({
  createChatModel: (model) => {
    modelCalls.push(model)
    return new FakeChatModel({})
  },
  buildPhaseAgent: () => ({
    invoke: () => invoke(),
  }),
})

const createContext = (
  call: (request: ExecutionToolRequest) => Promise<Result<unknown, Error>>,
  onStepsChanged?: (steps: readonly StrategyTaskStep[]) => void,
): RunAgentGraphContext => ({
  agentsMd: '',
  skills: {},
  originalPromptText: 'Process the provided synthetic input.',
  createStepMcpClient: () => {
    const client: McpToolsClient = {
      getTools: () => Promise.resolve([]),
      callExecutionTool: call,
      close: () => Promise.resolve(),
    }
    return client
  },
  ...(onStepsChanged !== undefined ? { onStepsChanged } : {}),
})

describe('runAgentGraph operation phases', () => {
  it('dispatches only registered tools for literal, prior-output, and dataset inputs without creating a model', async () => {
    const modelCalls: string[] = []
    const calls: ExecutionToolRequest[] = []
    const latestSteps: StrategyTaskStep[][] = []
    const context = createContext(
      (request) =>
        Promise.resolve().then(() => {
          calls.push(request)
          switch (request.name) {
            case 'execute_operation':
              return ok(
                calls.filter(
                  (candidate) => candidate.name === 'execute_operation',
                ).length === 1
                  ? operationResponse(
                      'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1',
                      'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1',
                      'demo-first-digest',
                    )
                  : operationResponse(
                      'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa2',
                      'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb2',
                      'demo-second-digest',
                    ),
              )
            case 'capture_dataset':
              return ok({
                payload: payload(
                  'cccccccc-cccc-4ccc-8ccc-ccccccccccc1',
                  'demo-dataset-digest',
                ),
              })
            case 'read_execution_payload':
            case 'read_task_execution_snapshot':
              return err(new Error('unexpected fixed tool call'))
          }
        }),
      (steps) => latestSteps.push([...steps]),
    )

    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'source',
          label: 'Prepare',
          kind: 'operation',
          operation: 'evaluate_saved_code',
          codeId: 'demo-source',
          input: { seed: { literal: { count: 3 } } },
          skills: [],
          subagents: [],
          output: { rows: { type: 'array' } },
        },
        {
          key: 'transform',
          label: 'Transform',
          kind: 'operation',
          operation: 'evaluate_saved_code',
          codeId: 'demo-transform',
          input: {
            mode: { literal: 'sample-mode' },
            rows: {
              priorOutput: {
                phase: 'source',
                pointer: '/value/rows',
                cursor: 2,
                limit: 3,
              },
            },
            dataset: {
              dataset: {
                provider: 'demo-source',
                params: { collection: 'demo-set' },
              },
            },
          },
          skills: [],
          subagents: [],
          output: { result: { type: 'object' } },
        },
      ],
    }

    const result = await runAgentGraph(
      createDeps(modelCalls, () => Promise.resolve({ structuredResponse: {} })),
      config,
      context,
    )

    expect(result).toEqual({
      status: 'completed',
      message: '2フェーズの実行が完了しました (Prepare → Transform)',
    })
    expect(modelCalls).toEqual([])
    expect(calls).toEqual([
      {
        name: 'execute_operation',
        arguments: {
          name: 'demo-source',
          args: { seed: { count: 3 } },
          payload_inputs: [],
        },
      },
      {
        name: 'capture_dataset',
        arguments: {
          provider: 'demo-source',
          params: { collection: 'demo-set' },
        },
      },
      {
        name: 'execute_operation',
        arguments: {
          name: 'demo-transform',
          args: { mode: 'sample-mode' },
          payload_inputs: [
            {
              field: 'rows',
              payload_id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1',
              pointer: '/value/rows',
              cursor: 2,
              limit: 3,
            },
            {
              field: 'dataset',
              payload_id: 'cccccccc-cccc-4ccc-8ccc-ccccccccccc1',
              pointer: '',
            },
          ],
        },
      },
    ])
    expect(normalizeStepTimestamps(latestSteps.at(-1) ?? [])).toEqual([
      {
        phaseKey: 'source',
        executionStepId: '<execution-step-id-1>',
        kind: 'operation',
        label: 'Prepare',
        model: null,
        status: 'completed',
        output: {
          payloadRef: {
            id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1',
            digest: 'demo-first-digest',
          },
          pointer: '/value',
        },
        inputRef: {
          payload_inputs: [],
          execution_input: {
            id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1',
            digest: 'demo-input-digest',
          },
        },
        outputRef: {
          id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1',
          digest: 'demo-first-digest',
        },
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
        traceId: NOOP_TRACE_ID,
        spanId: NOOP_SPAN_ID,
      },
      {
        phaseKey: 'transform',
        executionStepId: '<execution-step-id-2>',
        kind: 'operation',
        label: 'Transform',
        model: null,
        status: 'completed',
        output: {
          payloadRef: {
            id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb2',
            digest: 'demo-second-digest',
          },
          pointer: '/value',
        },
        inputRef: {
          payload_inputs: [
            {
              field: 'rows',
              payload_id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1',
              pointer: '/value/rows',
              cursor: 2,
              limit: 3,
            },
            {
              field: 'dataset',
              payload_id: 'cccccccc-cccc-4ccc-8ccc-ccccccccccc1',
              pointer: '',
            },
          ],
          execution_input: {
            id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa2',
            digest: 'demo-input-digest',
          },
        },
        outputRef: {
          id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb2',
          digest: 'demo-second-digest',
        },
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
        traceId: NOOP_TRACE_ID,
        spanId: NOOP_SPAN_ID,
      },
    ])
  })

  it('reads a sealed output in finite pages and resumes by the stable payload item key', async () => {
    const modelCalls: string[] = []
    const calls: ExecutionToolRequest[] = []
    const latestSteps: StrategyTaskStep[][] = []
    const payloadId = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1'
    const pointer = '/value/items'
    const previousSteps: StrategyTaskStep[] = [
      {
        phaseKey: 'prepare',
        executionStepId: 'old-source-step',
        kind: 'operation',
        label: 'Prepare',
        model: null,
        status: 'completed',
        output: {
          payloadRef: { id: payloadId, digest: 'demo-page-digest' },
          pointer: '/value',
        },
        outputRef: { id: payloadId, digest: 'demo-page-digest' },
        startedAt: '2026-01-01T00:00:00.000Z',
        finishedAt: '2026-01-01T00:00:01.000Z',
        traceId: 'old-source-trace',
        spanId: 'old-source-span',
      },
      {
        phaseKey: 'inspect',
        executionStepId: 'old-item-step',
        label: 'Inspect',
        model: 'demo-model',
        status: 'completed',
        item: { label: 'alpha' },
        itemKey: { index: 0, payload_id: payloadId, pointer },
        output: { inspected: 'alpha' },
        startedAt: '2026-01-01T00:00:02.000Z',
        finishedAt: '2026-01-01T00:00:03.000Z',
        traceId: 'old-item-trace',
        spanId: 'old-item-span',
      },
    ]
    const context = createContext(
      (request) =>
        Promise.resolve().then(() => {
          calls.push(request)
          if (request.name !== 'read_execution_payload') {
            return err(new Error('unexpected fixed tool call'))
          }
          const { cursor } = request.arguments
          return ok({
            payload_id: payloadId,
            pointer,
            data:
              cursor === 0
                ? [{ label: 'alpha' }, { label: 'beta' }]
                : [{ label: 'gamma' }],
            cursor,
            next_cursor: cursor === 0 ? 2 : null,
            total_count: 3,
            digest: 'demo-page-digest',
          })
        }),
      (steps) => latestSteps.push([...steps]),
    )
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'prepare',
          label: 'Prepare',
          kind: 'operation',
          operation: 'evaluate_saved_code',
          codeId: 'demo-transform',
          skills: [],
          subagents: [],
          output: { items: { type: 'array' } },
        },
        {
          key: 'inspect',
          label: 'Inspect',
          kind: 'llm',
          model: 'demo-model',
          prompt: 'Inspect the assigned item.',
          forEach: 'prepare.items',
          labelField: 'label',
          maxParallel: 2,
          skills: [],
          subagents: [],
          output: { inspected: { type: 'string' } },
        },
      ],
    }
    const deps = createDeps(modelCalls, () =>
      Promise.resolve({ structuredResponse: { inspected: 'complete' } }),
    )

    const result = await runAgentGraph(deps, config, {
      ...context,
      previousSteps,
    })

    expect(result).toEqual({
      status: 'completed',
      message: '2フェーズの実行が完了しました (Prepare → Inspect)',
    })
    expect(calls).toEqual([
      {
        name: 'read_execution_payload',
        arguments: {
          payload_id: payloadId,
          pointer,
          cursor: 0,
          limit: 2,
        },
      },
      {
        name: 'read_execution_payload',
        arguments: {
          payload_id: payloadId,
          pointer,
          cursor: 2,
          limit: 2,
        },
      },
    ])
    expect(modelCalls).toEqual(['demo-model', 'demo-model'])
    expect(normalizeStepTimestamps(latestSteps.at(-1) ?? [])).toEqual([
      {
        ...previousSteps[0],
        executionStepId: '<execution-step-id-1>',
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
      },
      {
        ...previousSteps[1],
        executionStepId: '<execution-step-id-2>',
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
      },
      {
        phaseKey: 'inspect',
        executionStepId: '<execution-step-id-3>',
        label: 'Inspect',
        model: 'demo-model',
        status: 'completed',
        item: { label: 'beta' },
        itemKey: { payload_id: payloadId, pointer, index: 1 },
        itemLabel: 'beta',
        output: { inspected: 'complete' },
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
        traceId: NOOP_TRACE_ID,
        spanId: NOOP_SPAN_ID,
      },
      {
        phaseKey: 'inspect',
        executionStepId: '<execution-step-id-4>',
        label: 'Inspect',
        model: 'demo-model',
        status: 'completed',
        item: { label: 'gamma' },
        itemKey: { payload_id: payloadId, pointer, index: 2 },
        itemLabel: 'gamma',
        output: { inspected: 'complete' },
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
        traceId: NOOP_TRACE_ID,
        spanId: NOOP_SPAN_ID,
      },
    ])
  })

  it('fails an operation phase when the saved code exits unsuccessfully', async () => {
    const modelCalls: string[] = []
    const calls: ExecutionToolRequest[] = []
    const latestSteps: StrategyTaskStep[][] = []
    const context = createContext(
      (request) =>
        Promise.resolve().then(() => {
          calls.push(request)
          return ok({
            input_payload: payload(
              'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1',
              'demo-input-digest',
            ),
            output_payload: payload(
              'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1',
              'demo-output-digest',
            ),
            exit_code: 7,
            stderr: 'synthetic failure',
          })
        }),
      (steps) => latestSteps.push([...steps]),
    )
    const result = await runAgentGraph(
      createDeps(modelCalls, () => Promise.resolve({ structuredResponse: {} })),
      {
        phases: [
          {
            key: 'prepare',
            label: 'Prepare',
            kind: 'operation',
            operation: 'evaluate_saved_code',
            codeId: 'demo-transform',
            skills: [],
            subagents: [],
            output: {},
          },
        ],
      },
      context,
    )

    expect(result).toEqual({
      status: 'failed',
      message:
        'フェーズ「Prepare」(prepare) の実行に失敗しました: saved operation exited with status 7: synthetic failure',
      errorKind: 'agent_error',
    })
    expect(modelCalls).toEqual([])
    expect(calls).toEqual([
      {
        name: 'execute_operation',
        arguments: {
          name: 'demo-transform',
          args: {},
          payload_inputs: [],
        },
      },
    ])
    expect(normalizeStepTimestamps(latestSteps.at(-1) ?? [])).toEqual([
      {
        phaseKey: 'prepare',
        executionStepId: '<execution-step-id-1>',
        kind: 'operation',
        label: 'Prepare',
        model: null,
        status: 'failed',
        error: 'saved operation exited with status 7: synthetic failure',
        inputRef: {
          payload_inputs: [],
          execution_input: {
            id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1',
            digest: 'demo-input-digest',
          },
        },
        outputRef: {
          id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1',
          digest: 'demo-output-digest',
        },
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
        traceId: NOOP_TRACE_ID,
        spanId: NOOP_SPAN_ID,
      },
    ])
  })

  it('reuses captured dataset payloads when resuming an operation step', async () => {
    const modelCalls: string[] = []
    const calls: ExecutionToolRequest[] = []
    const firstSteps: StrategyTaskStep[][] = []
    const resumedSteps: StrategyTaskStep[][] = []
    const operationInputSnapshots: StrategyTaskStep[][] = []
    const datasetPayload = payload(
      'cccccccc-cccc-4ccc-8ccc-ccccccccccc1',
      'demo-dataset-digest',
    )
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'prepare',
          label: 'Prepare',
          kind: 'operation',
          operation: 'evaluate_saved_code',
          codeId: 'demo-transform',
          input: {
            rows: {
              dataset: {
                provider: 'demo-source',
                params: { collection: 'demo-set' },
              },
            },
          },
          skills: [],
          subagents: [],
          output: {},
        },
      ],
    }
    const firstContext = createContext(
      (request) =>
        Promise.resolve().then(() => {
          calls.push(request)
          if (request.name === 'capture_dataset')
            return ok({ payload: datasetPayload })
          if (request.name === 'execute_operation') {
            operationInputSnapshots.push(firstSteps.at(-1) ?? [])
            return err(new Error('synthetic transport failure'))
          }
          return err(new Error('unexpected fixed tool call'))
        }),
      (steps) => firstSteps.push([...steps]),
    )

    const firstResult = await runAgentGraph(
      createDeps(modelCalls, () => Promise.resolve({ structuredResponse: {} })),
      config,
      firstContext,
    )
    const previousSteps = firstSteps.at(-1) ?? []
    const resumedContext = createContext(
      (request) =>
        Promise.resolve().then(() => {
          calls.push(request)
          if (request.name === 'execute_operation') {
            operationInputSnapshots.push(resumedSteps.at(-1) ?? [])
            return ok(
              operationResponse(
                'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1',
                'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1',
                'demo-output-digest',
              ),
            )
          }
          return err(new Error('dataset capture must be reused'))
        }),
      (steps) => resumedSteps.push([...steps]),
    )

    const resumedResult = await runAgentGraph(
      createDeps(modelCalls, () => Promise.resolve({ structuredResponse: {} })),
      config,
      { ...resumedContext, previousSteps },
    )

    expect(firstResult).toEqual({
      status: 'failed',
      message:
        'フェーズ「Prepare」(prepare) の実行に失敗しました: synthetic transport failure',
      errorKind: 'agent_error',
    })
    expect(resumedResult).toEqual({
      status: 'completed',
      message: '1フェーズの実行が完了しました (Prepare)',
    })
    expect(calls).toEqual([
      {
        name: 'capture_dataset',
        arguments: {
          provider: 'demo-source',
          params: { collection: 'demo-set' },
        },
      },
      {
        name: 'execute_operation',
        arguments: {
          name: 'demo-transform',
          args: {},
          payload_inputs: [
            {
              field: 'rows',
              payload_id: 'cccccccc-cccc-4ccc-8ccc-ccccccccccc1',
              pointer: '',
            },
          ],
        },
      },
      {
        name: 'execute_operation',
        arguments: {
          name: 'demo-transform',
          args: {},
          payload_inputs: [
            {
              field: 'rows',
              payload_id: 'cccccccc-cccc-4ccc-8ccc-ccccccccccc1',
              pointer: '',
            },
          ],
        },
      },
    ])
    expect(
      operationInputSnapshots.map((steps) => normalizeStepTimestamps(steps)),
    ).toEqual([
      [
        {
          phaseKey: 'prepare',
          executionStepId: '<execution-step-id-1>',
          kind: 'operation',
          label: 'Prepare',
          model: null,
          status: 'running',
          inputRef: {
            payload_inputs: [
              {
                field: 'rows',
                payload_id: 'cccccccc-cccc-4ccc-8ccc-ccccccccccc1',
                pointer: '',
              },
            ],
          },
          startedAt: '<started-at>',
          traceId: NOOP_TRACE_ID,
          spanId: NOOP_SPAN_ID,
        },
      ],
      [
        {
          phaseKey: 'prepare',
          executionStepId: '<execution-step-id-1>',
          kind: 'operation',
          label: 'Prepare',
          model: null,
          status: 'running',
          inputRef: {
            payload_inputs: [
              {
                field: 'rows',
                payload_id: 'cccccccc-cccc-4ccc-8ccc-ccccccccccc1',
                pointer: '',
              },
            ],
          },
          startedAt: '<started-at>',
          traceId: NOOP_TRACE_ID,
          spanId: NOOP_SPAN_ID,
        },
      ],
    ])
    expect(normalizeStepTimestamps(resumedSteps.at(-1) ?? [])).toEqual([
      {
        phaseKey: 'prepare',
        executionStepId: '<execution-step-id-1>',
        kind: 'operation',
        label: 'Prepare',
        model: null,
        status: 'completed',
        inputRef: {
          payload_inputs: [
            {
              field: 'rows',
              payload_id: 'cccccccc-cccc-4ccc-8ccc-ccccccccccc1',
              pointer: '',
            },
          ],
          execution_input: {
            id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1',
            digest: 'demo-input-digest',
          },
        },
        outputRef: {
          id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1',
          digest: 'demo-output-digest',
        },
        output: {
          payloadRef: {
            id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1',
            digest: 'demo-output-digest',
          },
          pointer: '/value',
        },
        startedAt: '<started-at>',
        finishedAt: '<finished-at>',
        traceId: NOOP_TRACE_ID,
        spanId: NOOP_SPAN_ID,
      },
    ])
    expect(modelCalls).toEqual([])
  })
})
