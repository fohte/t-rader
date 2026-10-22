import type { Result } from 'neverthrow'
import { err, ok } from 'neverthrow'

import { isPlainObject } from '#strategy-agent/agent-graph/json'
import type {
  RunAgentGraphContext,
  StepRecorder,
} from '#strategy-agent/agent-graph/run-agent-graph/types'
import { withPhaseSpan } from '#strategy-agent/agent-graph/tracing'
import type {
  InputBinding,
  OperationAgentGraphPhase,
} from '#strategy-agent/agent-graph/types'
import type { ExecutionToolRequest } from '#strategy-agent/execution-tool-client'
import type { McpToolsClient } from '#strategy-agent/strategy-agent'

export interface PayloadReference {
  readonly id: string
  readonly digest?: string
}

export interface OperationPhaseOutput {
  readonly payloadRef: PayloadReference
  readonly pointer: '/value'
}

interface PayloadInput {
  readonly field: string
  readonly payload_id: string
  readonly pointer: string
  readonly cursor?: number
  readonly limit?: number
}

interface ResolvedOperationInput {
  readonly args: Readonly<Record<string, unknown>>
  readonly payloadInputs: readonly PayloadInput[]
}

interface StoredOperationInputReference {
  readonly payload_inputs: readonly PayloadInput[]
  readonly execution_input?: unknown
}

const isPayloadInput = (value: unknown): value is PayloadInput =>
  isPlainObject(value) &&
  typeof value['field'] === 'string' &&
  typeof value['payload_id'] === 'string' &&
  typeof value['pointer'] === 'string' &&
  (value['cursor'] === undefined ||
    (typeof value['cursor'] === 'number' &&
      Number.isSafeInteger(value['cursor']) &&
      value['cursor'] >= 0)) &&
  (value['limit'] === undefined ||
    (typeof value['limit'] === 'number' &&
      Number.isSafeInteger(value['limit']) &&
      value['limit'] >= 1 &&
      value['limit'] <= 1_000))

const extractStoredPayloadInputs = (value: unknown): readonly PayloadInput[] =>
  isPlainObject(value) && Array.isArray(value['payload_inputs'])
    ? value['payload_inputs'].filter(isPayloadInput)
    : []

const buildInputReference = (
  payloadInputs: readonly PayloadInput[],
  executionInput?: unknown,
): StoredOperationInputReference => ({
  payload_inputs: payloadInputs,
  ...(executionInput !== undefined ? { execution_input: executionInput } : {}),
})

const isPayloadReference = (value: unknown): value is PayloadReference =>
  isPlainObject(value) &&
  typeof value['id'] === 'string' &&
  (value['digest'] === undefined || typeof value['digest'] === 'string')

export const extractPayloadReference = (
  value: unknown,
): PayloadReference | undefined => {
  if (!isPlainObject(value)) return undefined
  if (isPayloadReference(value)) return value
  for (const key of [
    'payloadRef',
    'payload_ref',
    'output_payload',
    'payload',
  ]) {
    const nested = value[key]
    if (isPayloadReference(nested)) return nested
  }
  return undefined
}

const callExecutionTool = (
  client: McpToolsClient,
  request: ExecutionToolRequest,
): Promise<Result<unknown, Error>> =>
  client.callExecutionTool === undefined
    ? Promise.resolve(
        err(new Error('execution MCP tool dispatcher is not configured')),
      )
    : Promise.resolve()
        .then(() => client.callExecutionTool?.(request))
        .then(
          (result) =>
            result ??
            err(new Error('execution MCP tool dispatcher is not configured')),
        )
        .catch((error: unknown) =>
          err(error instanceof Error ? error : new Error(String(error))),
        )

const resolvePayloadInput = (
  field: string,
  binding: Exclude<InputBinding, { readonly literal: unknown }>,
  priorResults: Readonly<Record<string, unknown>>,
  client: McpToolsClient,
): Promise<Result<PayloadInput, Error>> => {
  if ('priorOutput' in binding) {
    const prior = priorResults[binding.priorOutput.phase]
    const payloadRef = extractPayloadReference(prior)
    return Promise.resolve(
      payloadRef === undefined
        ? err(
            new Error(
              `prior_output phase "${binding.priorOutput.phase}" has no sealed payload`,
            ),
          )
        : ok({
            field,
            payload_id: payloadRef.id,
            pointer: binding.priorOutput.pointer,
            ...(binding.priorOutput.cursor !== undefined
              ? { cursor: binding.priorOutput.cursor }
              : {}),
            ...(binding.priorOutput.limit !== undefined
              ? { limit: binding.priorOutput.limit }
              : {}),
          }),
    )
  }

  return callExecutionTool(client, {
    name: 'capture_dataset',
    arguments: {
      provider: binding.dataset.provider,
      params: binding.dataset.params,
    },
  }).then((result) =>
    result.andThen((captured) => {
      const payloadRef = extractPayloadReference(captured)
      return payloadRef === undefined
        ? err(new Error('capture_dataset returned no payload reference'))
        : ok({ field, payload_id: payloadRef.id, pointer: '' })
    }),
  )
}

const resolveOperationInput = async (
  input: Readonly<Record<string, InputBinding>> | undefined,
  priorResults: Readonly<Record<string, unknown>>,
  client: McpToolsClient,
  context: RunAgentGraphContext,
  previousPayloadInputs: readonly PayloadInput[],
  recorder: StepRecorder,
  stepIndex: number,
): Promise<Result<ResolvedOperationInput, Error>> => {
  const args: Record<string, unknown> = {}
  const payloadInputFields = Object.entries(input ?? {})
    .filter(([, binding]) => !('literal' in binding))
    .map(([field]) => field)
  const payloadInputsByField = new Map(
    previousPayloadInputs
      .filter((payloadInput) => payloadInputFields.includes(payloadInput.field))
      .map((payloadInput) => [payloadInput.field, payloadInput]),
  )
  const orderedPayloadInputs = (): PayloadInput[] =>
    payloadInputFields.flatMap((field) => {
      const payloadInput = payloadInputsByField.get(field)
      return payloadInput === undefined ? [] : [payloadInput]
    })

  for (const [field, binding] of Object.entries(input ?? {})) {
    if (context.deadlineSignal?.aborted === true) {
      return err(new Error('phase deadline exceeded'))
    }
    if ('literal' in binding) {
      args[field] = binding.literal
      continue
    }
    if (payloadInputsByField.has(field)) continue
    const result = await resolvePayloadInput(
      field,
      binding,
      priorResults,
      client,
    )
    if (result.isErr()) return err(result.error)
    payloadInputsByField.set(field, result.value)
    recorder.update(stepIndex, {
      inputRef: buildInputReference(orderedPayloadInputs()),
    })
  }
  return ok({ args, payloadInputs: orderedPayloadInputs() })
}

const isOperationResult = (
  value: unknown,
): value is {
  readonly input_payload: unknown
  readonly output_payload: unknown
  readonly exit_code: number
  readonly stderr: string
} =>
  isPlainObject(value) &&
  typeof value['exit_code'] === 'number' &&
  typeof value['stderr'] === 'string' &&
  'input_payload' in value &&
  'output_payload' in value

const runOperation = async (
  phase: OperationAgentGraphPhase,
  context: RunAgentGraphContext,
  priorResults: Readonly<Record<string, unknown>>,
  recorder: StepRecorder,
  existingExecutionStepId?: string,
  previousInputRef?: unknown,
): Promise<Result<OperationPhaseOutput, unknown>> =>
  withPhaseSpan(
    phase.label,
    { 'phase.key': phase.key, 'phase.operation': phase.operation },
    (spanIds) => {
      const executionStepId = existingExecutionStepId ?? crypto.randomUUID()
      const previousPayloadInputs = extractStoredPayloadInputs(previousInputRef)
      const stepIndex = recorder.start({
        phaseKey: phase.key,
        executionStepId,
        kind: 'operation',
        label: phase.label,
        model: null,
        startedAt: new Date().toISOString(),
        ...(previousPayloadInputs.length > 0
          ? { inputRef: buildInputReference(previousPayloadInputs) }
          : {}),
        traceId: spanIds.traceId,
        spanId: spanIds.spanId,
      })
      return Promise.resolve()
        .then(() => context.createStepMcpClient(executionStepId))
        .then((client) =>
          Promise.resolve()
            .then(async () => {
              const input = await resolveOperationInput(
                phase.input,
                priorResults,
                client,
                context,
                previousPayloadInputs,
                recorder,
                stepIndex,
              )
              if (input.isErr()) return err(input.error)
              if (context.deadlineSignal?.aborted === true) {
                return err(new Error('phase deadline exceeded'))
              }
              const executed = await callExecutionTool(client, {
                name: 'execute_operation',
                arguments: {
                  name: phase.codeId,
                  args: input.value.args,
                  payload_inputs: input.value.payloadInputs,
                },
              })
              return executed.andThen((raw) => {
                if (!isOperationResult(raw)) {
                  return err(
                    new Error('execute_operation returned a malformed result'),
                  )
                }
                const inputRef = buildInputReference(
                  input.value.payloadInputs,
                  raw.input_payload,
                )
                recorder.update(stepIndex, {
                  inputRef,
                  outputRef: raw.output_payload,
                })
                if (raw.exit_code !== 0) {
                  return err(
                    new Error(
                      `saved operation exited with status ${String(raw.exit_code)}: ${raw.stderr}`,
                    ),
                  )
                }
                const payloadRef = extractPayloadReference(raw.output_payload)
                if (payloadRef === undefined) {
                  return err(
                    new Error('execute_operation returned no output payload'),
                  )
                }
                return ok({
                  output: { payloadRef, pointer: '/value' as const },
                  inputRef,
                  outputRef: raw.output_payload,
                })
              })
            })
            .finally(() => client.close().catch(() => undefined)),
        )
        .catch((error: unknown) =>
          err(error instanceof Error ? error : new Error(String(error))),
        )
        .then((result) => {
          recorder.finish(
            stepIndex,
            result.isErr()
              ? { status: 'failed', error: result.error.message }
              : {
                  status: 'completed',
                  output: result.value.output,
                  inputRef: result.value.inputRef,
                  outputRef: result.value.outputRef,
                },
          )
          return result.map((value) => value.output)
        })
    },
  )

export const runOperationPhase = (
  phase: OperationAgentGraphPhase,
  context: RunAgentGraphContext,
  priorResults: Readonly<Record<string, unknown>>,
  recorder: StepRecorder,
  existingExecutionStepId?: string,
  previousInputRef?: unknown,
): Promise<Result<OperationPhaseOutput, unknown>> =>
  runOperation(
    phase,
    context,
    priorResults,
    recorder,
    existingExecutionStepId,
    previousInputRef,
  )
