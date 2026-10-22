import { BaseChatModel } from '@langchain/core/language_models/chat_models'
import type { ChatResult } from '@langchain/core/outputs'
import type { Result } from 'neverthrow'
import { err, ok } from 'neverthrow'
import { describe, expect, it, vi } from 'vitest'

import { runAgentGraph } from '#strategy-agent/agent-graph/run-agent-graph'
import { buildPhaseMessageText } from '#strategy-agent/agent-graph/run-agent-graph/phase-message'
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
  call: (
    request: ExecutionToolRequest,
    signal?: AbortSignal,
  ) => Promise<Result<unknown, Error>>,
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

    const actual = Array.of<unknown>(
      result,
      modelCalls,
      calls,
      normalizeStepTimestamps(latestSteps.at(-1) ?? []),
    )

    expect(actual).toEqual([
      {
        status: 'completed',
        message: '2フェーズの実行が完了しました (Prepare → Transform)',
      },
      [],
      [
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
      ],
      [
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
      ],
    ])
  })

  it('binds a sealed prior-output page into an LLM phase message', async () => {
    const payloadId = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1'
    const pageDigest = 'demo-page-digest'
    const calls: ExecutionToolRequest[] = []
    const modelCalls: string[] = []
    const messages: string[] = []
    const context = createContext((request) => {
      calls.push(request)
      switch (request.name) {
        case 'execute_operation':
          return Promise.resolve(
            ok(
              operationResponse(
                'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1',
                payloadId,
                'demo-output-digest',
              ),
            ),
          )
        case 'read_execution_payload':
          return Promise.resolve(
            ok({
              payload_id: payloadId,
              pointer: '/value/rows',
              data: [{ value: 7 }],
              cursor: 2,
              next_cursor: null,
              total_count: 3,
              digest: pageDigest,
            }),
          )
        case 'capture_dataset':
        case 'read_task_execution_snapshot':
          return Promise.resolve(err(new Error('unexpected fixed tool call')))
      }
    })
    const deps: RunAgentGraphDeps = {
      createChatModel: (model) => {
        modelCalls.push(model)
        return new FakeChatModel({})
      },
      buildPhaseAgent: () => ({
        invoke: ({ messages: phaseMessages }) => {
          messages.push(phaseMessages.map((message) => message.text).join('\n'))
          return Promise.resolve({ structuredResponse: { review: 'complete' } })
        },
      }),
    }
    const priorResult = {
      payloadRef: { id: payloadId, digest: 'demo-output-digest' },
      pointer: '/value',
    }
    const resolvedInput = {
      payload_id: payloadId,
      pointer: '/value/rows',
      cursor: 2,
      data: [{ value: 7 }],
      next_cursor: null,
      total_count: 3,
      digest: pageDigest,
    }
    const config: AgentGraphConfig = {
      phases: [
        {
          key: 'prepare',
          label: 'Prepare',
          kind: 'operation',
          operation: 'evaluate_saved_code',
          codeId: 'demo-source',
          skills: [],
          subagents: [],
          output: { rows: { type: 'array' } },
        },
        {
          key: 'review',
          label: 'Review',
          kind: 'llm',
          model: 'demo-model',
          prompt: 'Review the captured values.',
          input: {
            rows: {
              priorOutput: {
                phase: 'prepare',
                pointer: '/value/rows',
                cursor: 2,
                limit: 2,
              },
            },
          },
          skills: [],
          subagents: [],
          output: { review: { type: 'string' } },
        },
      ],
    }

    const result = await runAgentGraph(deps, config, context)
    const actual = Array.of<unknown>(result, calls, modelCalls, messages)

    expect(actual).toEqual([
      {
        status: 'completed',
        message: '2フェーズの実行が完了しました (Prepare → Review)',
      },
      [
        {
          name: 'execute_operation',
          arguments: {
            name: 'demo-source',
            args: {},
            payload_inputs: [],
          },
        },
        {
          name: 'read_execution_payload',
          arguments: {
            payload_id: payloadId,
            pointer: '/value/rows',
            cursor: 2,
            limit: 2,
          },
        },
      ],
      ['demo-model'],
      [
        buildPhaseMessageText({
          originalPromptText: 'Process the provided synthetic input.',
          phaseKeys: ['prepare', 'review'],
          phaseKey: 'review',
          phasePrompt: 'Review the captured values.',
          item: undefined,
          priorResults: { prepare: priorResult },
          inputBindings: { rows: resolvedInput },
        }),
      ],
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
          limit: 100,
        },
      },
      {
        name: 'read_execution_payload',
        arguments: {
          payload_id: payloadId,
          pointer,
          cursor: 2,
          limit: 100,
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

  it('cancels an in-flight operation at its phase timeout and skips later phases', async () => {
    vi.useFakeTimers()
    try {
      const modelCalls: string[] = []
      const calls: ExecutionToolRequest[] = []
      const latestSteps: StrategyTaskStep[][] = []
      let operationSignal: AbortSignal | undefined
      let markOperationStarted: () => void = () => undefined
      const operationStarted = new Promise<void>((resolve) => {
        markOperationStarted = () => {
          resolve()
        }
      })
      const context = createContext(
        (request, signal) => {
          calls.push(request)
          operationSignal = signal
          markOperationStarted()
          return new Promise((_, reject) => {
            if (signal === undefined) {
              reject(new Error('operation request had no cancellation signal'))
              return
            }
            signal.addEventListener(
              'abort',
              () => {
                reject(new Error('operation request cancelled'))
              },
              { once: true },
            )
          })
        },
        (steps) => latestSteps.push([...steps]),
      )
      const execution = runAgentGraph(
        createDeps(modelCalls, () =>
          Promise.resolve({ structuredResponse: {} }),
        ),
        {
          phases: [
            {
              key: 'prepare',
              label: 'Prepare',
              kind: 'operation',
              operation: 'evaluate_saved_code',
              codeId: 'demo-transform',
              timeoutMinutes: 1,
              skills: [],
              subagents: [],
              output: {},
            },
            {
              key: 'follow-up',
              label: 'Follow up',
              kind: 'llm',
              model: 'demo-follow-up-model',
              prompt: 'Continue after the operation.',
              skills: [],
              subagents: [],
              output: {},
            },
          ],
        },
        context,
      )

      await operationStarted
      await vi.advanceTimersByTimeAsync(60_000)
      const result = await execution
      expect(
        Array.of<unknown>(
          result,
          calls,
          modelCalls,
          operationSignal?.aborted ?? false,
          normalizeStepTimestamps(latestSteps.at(-1) ?? []),
        ),
      ).toEqual([
        {
          status: 'failed',
          message: 'フェーズ prepare が timeout (1 分) を超過',
          errorKind: 'agent_error',
        },
        [
          {
            name: 'execute_operation',
            arguments: {
              name: 'demo-transform',
              args: {},
              payload_inputs: [],
              timeout_secs: 60,
            },
          },
        ],
        [],
        true,
        [
          {
            phaseKey: 'prepare',
            executionStepId: '<execution-step-id-1>',
            kind: 'operation',
            label: 'Prepare',
            model: null,
            status: 'failed',
            error: 'operation request cancelled',
            startedAt: '<started-at>',
            finishedAt: '<finished-at>',
            traceId: NOOP_TRACE_ID,
            spanId: NOOP_SPAN_ID,
          },
        ],
      ])
    } finally {
      vi.useRealTimers()
    }
  })

  it('cancels a prior-output payload read at its phase timeout before later phases run', async () => {
    vi.useFakeTimers()
    try {
      const modelCalls: string[] = []
      const calls: ExecutionToolRequest[] = []
      let payloadReadSignal: AbortSignal | undefined
      let markPayloadReadStarted: () => void = () => undefined
      const payloadReadStarted = new Promise<void>((resolve) => {
        markPayloadReadStarted = () => {
          resolve()
        }
      })
      const payloadId = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1'
      const context = createContext((request, signal) => {
        calls.push(request)
        if (request.name === 'execute_operation') {
          return Promise.resolve(
            ok(
              operationResponse(
                'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1',
                payloadId,
                'demo-output-digest',
              ),
            ),
          )
        }
        if (request.name !== 'read_execution_payload') {
          return Promise.resolve(err(new Error('unexpected fixed tool call')))
        }
        payloadReadSignal = signal
        markPayloadReadStarted()
        return new Promise((_, reject) => {
          if (signal === undefined) {
            reject(new Error('payload read request had no cancellation signal'))
            return
          }
          signal.addEventListener(
            'abort',
            () => {
              reject(new Error('payload read request cancelled'))
            },
            { once: true },
          )
        })
      })
      const execution = runAgentGraph(
        createDeps(modelCalls, () =>
          Promise.resolve({ structuredResponse: {} }),
        ),
        {
          phases: [
            {
              key: 'source',
              label: 'Source',
              kind: 'operation',
              operation: 'evaluate_saved_code',
              codeId: 'demo-source',
              skills: [],
              subagents: [],
              output: { items: { type: 'array' } },
            },
            {
              key: 'consume',
              label: 'Consume',
              kind: 'llm',
              model: 'demo-consumer-model',
              prompt: 'Read the selected payload page.',
              timeoutMinutes: 1,
              input: {
                rows: {
                  priorOutput: {
                    phase: 'source',
                    pointer: '/value/items',
                    cursor: 0,
                    limit: 100,
                  },
                },
              },
              skills: [],
              subagents: [],
              output: {},
            },
            {
              key: 'follow-up',
              label: 'Follow up',
              kind: 'llm',
              model: 'demo-follow-up-model',
              prompt: 'Continue after the payload read.',
              skills: [],
              subagents: [],
              output: {},
            },
          ],
        },
        context,
      )

      await payloadReadStarted
      await vi.advanceTimersByTimeAsync(60_000)
      const result = await execution
      expect(
        Array.of<unknown>(
          result,
          calls,
          modelCalls,
          payloadReadSignal?.aborted ?? false,
        ),
      ).toEqual([
        {
          status: 'failed',
          message: 'フェーズ consume が timeout (1 分) を超過',
          errorKind: 'agent_error',
        },
        [
          {
            name: 'execute_operation',
            arguments: {
              name: 'demo-source',
              args: {},
              payload_inputs: [],
            },
          },
          {
            name: 'read_execution_payload',
            arguments: {
              payload_id: payloadId,
              pointer: '/value/items',
              cursor: 0,
              limit: 100,
            },
          },
        ],
        [],
        true,
      ])
    } finally {
      vi.useRealTimers()
    }
  })

  it('cancels an in-flight dataset capture before the operation and later phases run', async () => {
    vi.useFakeTimers()
    try {
      const modelCalls: string[] = []
      const calls: ExecutionToolRequest[] = []
      const latestSteps: StrategyTaskStep[][] = []
      let captureSignal: AbortSignal | undefined
      let markCaptureStarted: () => void = () => undefined
      const captureStarted = new Promise<void>((resolve) => {
        markCaptureStarted = () => {
          resolve()
        }
      })
      const context = createContext(
        (request, signal) => {
          calls.push(request)
          if (request.name !== 'capture_dataset') {
            return Promise.resolve(err(new Error('unexpected fixed tool call')))
          }
          captureSignal = signal
          markCaptureStarted()
          return new Promise((_, reject) => {
            if (signal === undefined) {
              reject(new Error('dataset capture had no cancellation signal'))
              return
            }
            signal.addEventListener(
              'abort',
              () => {
                reject(new Error('dataset capture cancelled'))
              },
              { once: true },
            )
          })
        },
        (steps) => latestSteps.push([...steps]),
      )
      const execution = runAgentGraph(
        createDeps(modelCalls, () =>
          Promise.resolve({ structuredResponse: {} }),
        ),
        {
          phases: [
            {
              key: 'prepare',
              label: 'Prepare',
              kind: 'operation',
              operation: 'evaluate_saved_code',
              codeId: 'demo-transform',
              timeoutMinutes: 1,
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
            {
              key: 'follow-up',
              label: 'Follow up',
              kind: 'llm',
              model: 'demo-follow-up-model',
              prompt: 'Continue after the operation.',
              skills: [],
              subagents: [],
              output: {},
            },
          ],
        },
        context,
      )

      await captureStarted
      await vi.advanceTimersByTimeAsync(60_000)
      const result = await execution
      expect(
        Array.of<unknown>(
          result,
          calls,
          modelCalls,
          captureSignal?.aborted ?? false,
          normalizeStepTimestamps(latestSteps.at(-1) ?? []),
        ),
      ).toEqual([
        {
          status: 'failed',
          message: 'フェーズ prepare が timeout (1 分) を超過',
          errorKind: 'agent_error',
        },
        [
          {
            name: 'capture_dataset',
            arguments: {
              provider: 'demo-source',
              params: { collection: 'demo-set' },
            },
          },
        ],
        [],
        true,
        [
          {
            phaseKey: 'prepare',
            executionStepId: '<execution-step-id-1>',
            kind: 'operation',
            label: 'Prepare',
            model: null,
            status: 'failed',
            error: 'dataset capture cancelled',
            startedAt: '<started-at>',
            finishedAt: '<finished-at>',
            traceId: NOOP_TRACE_ID,
            spanId: NOOP_SPAN_ID,
          },
        ],
      ])
    } finally {
      vi.useRealTimers()
    }
  })

  it('stops payload-backed for_each after cancellation and preserves completed items', async () => {
    vi.useFakeTimers()
    try {
      const modelCalls: string[] = []
      const calls: ExecutionToolRequest[] = []
      const latestSteps: StrategyTaskStep[][] = []
      const payloadId = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1'
      const pointer = '/value/items'
      let payloadReadSignal: AbortSignal | undefined
      let markSecondPageStarted: () => void = () => undefined
      const secondPageStarted = new Promise<void>((resolve) => {
        markSecondPageStarted = () => {
          resolve()
        }
      })
      const context = createContext(
        (request, signal) => {
          calls.push(request)
          if (request.name === 'execute_operation') {
            return Promise.resolve(
              ok(
                operationResponse(
                  'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1',
                  payloadId,
                  'demo-page-digest',
                ),
              ),
            )
          }
          if (request.name !== 'read_execution_payload') {
            return Promise.resolve(err(new Error('unexpected fixed tool call')))
          }
          const { cursor } = request.arguments
          if (cursor === 0) {
            return Promise.resolve(
              ok({
                payload_id: payloadId,
                pointer,
                data: [{ label: 'first-item' }],
                cursor,
                next_cursor: 1,
                total_count: 2,
                digest: 'demo-page-digest',
              }),
            )
          }
          payloadReadSignal = signal
          markSecondPageStarted()
          return new Promise((_, reject) => {
            if (signal === undefined) {
              reject(new Error('payload page read had no cancellation signal'))
              return
            }
            signal.addEventListener(
              'abort',
              () => {
                reject(new Error('payload page read cancelled'))
              },
              { once: true },
            )
          })
        },
        (steps) => latestSteps.push([...steps]),
      )
      const execution = runAgentGraph(
        createDeps(modelCalls, () =>
          Promise.resolve({ structuredResponse: { inspected: 'complete' } }),
        ),
        {
          phases: [
            {
              key: 'prepare',
              label: 'Prepare',
              kind: 'operation',
              operation: 'evaluate_saved_code',
              codeId: 'demo-source',
              skills: [],
              subagents: [],
              output: { items: { type: 'array' } },
            },
            {
              key: 'inspect',
              label: 'Inspect',
              kind: 'llm',
              model: 'demo-item-model',
              prompt: 'Inspect the assigned item.',
              forEach: 'prepare.items',
              labelField: 'label',
              maxParallel: 1,
              timeoutMinutes: 1,
              skills: [],
              subagents: [],
              output: { inspected: { type: 'string' } },
            },
            {
              key: 'follow-up',
              label: 'Follow up',
              kind: 'llm',
              model: 'demo-follow-up-model',
              prompt: 'Continue after item inspection.',
              skills: [],
              subagents: [],
              output: {},
            },
          ],
        },
        context,
      )

      await secondPageStarted
      await vi.advanceTimersByTimeAsync(60_000)
      const result = await execution
      expect(
        Array.of<unknown>(
          result,
          calls,
          modelCalls,
          payloadReadSignal?.aborted ?? false,
          normalizeStepTimestamps(latestSteps.at(-1) ?? []),
        ),
      ).toEqual([
        {
          status: 'failed',
          message: 'フェーズ inspect が timeout (1 分) を超過',
          errorKind: 'agent_error',
        },
        [
          {
            name: 'execute_operation',
            arguments: {
              name: 'demo-source',
              args: {},
              payload_inputs: [],
            },
          },
          {
            name: 'read_execution_payload',
            arguments: {
              payload_id: payloadId,
              pointer,
              cursor: 0,
              limit: 100,
            },
          },
          {
            name: 'read_execution_payload',
            arguments: {
              payload_id: payloadId,
              pointer,
              cursor: 1,
              limit: 100,
            },
          },
        ],
        ['demo-item-model'],
        true,
        [
          {
            phaseKey: 'prepare',
            executionStepId: '<execution-step-id-1>',
            kind: 'operation',
            label: 'Prepare',
            model: null,
            status: 'completed',
            output: {
              payloadRef: {
                id: payloadId,
                digest: 'demo-page-digest',
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
              id: payloadId,
              digest: 'demo-page-digest',
            },
            startedAt: '<started-at>',
            finishedAt: '<finished-at>',
            traceId: NOOP_TRACE_ID,
            spanId: NOOP_SPAN_ID,
          },
          {
            phaseKey: 'inspect',
            executionStepId: '<execution-step-id-2>',
            label: 'Inspect',
            model: 'demo-item-model',
            status: 'completed',
            item: { label: 'first-item' },
            itemKey: { payload_id: payloadId, pointer, index: 0 },
            itemLabel: 'first-item',
            output: { inspected: 'complete' },
            startedAt: '<started-at>',
            finishedAt: '<finished-at>',
            traceId: NOOP_TRACE_ID,
            spanId: NOOP_SPAN_ID,
          },
        ],
      ])
    } finally {
      vi.useRealTimers()
    }
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
