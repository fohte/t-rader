import type { Result } from 'neverthrow'
import { err, ok } from 'neverthrow'

import { extractPayloadReference } from '#strategy-agent/agent-graph/run-agent-graph/operation-runner'
import { readSealedPayloadPage } from '#strategy-agent/agent-graph/run-agent-graph/payload-for-each'
import type { RunAgentGraphContext } from '#strategy-agent/agent-graph/run-agent-graph/types'
import type { InputBinding } from '#strategy-agent/agent-graph/types'

const resolvePayloadBinding = async (
  binding: Exclude<InputBinding, { readonly literal: unknown }>,
  priorResults: Readonly<Record<string, unknown>>,
  context: RunAgentGraphContext,
): Promise<Result<unknown, Error>> => {
  if ('dataset' in binding) {
    return err(
      new Error('dataset bindings are only supported by operation phases'),
    )
  }
  if (!('priorOutput' in binding)) {
    return err(new Error('unsupported LLM input binding'))
  }
  const payloadRef = extractPayloadReference(
    priorResults[binding.priorOutput.phase],
  )
  if (payloadRef === undefined) {
    return err(
      new Error(
        `prior_output phase "${binding.priorOutput.phase}" has no sealed payload`,
      ),
    )
  }
  return Promise.resolve()
    .then(() => context.createStepMcpClient())
    .then((client) =>
      readSealedPayloadPage(
        client,
        payloadRef,
        binding.priorOutput.pointer,
        binding.priorOutput.cursor ?? 0,
        binding.priorOutput.limit ?? 100,
      )
        .finally(() => client.close().catch(() => undefined))
        .then((page) =>
          page.map((value) => ({
            payload_id: payloadRef.id,
            pointer: binding.priorOutput.pointer,
            cursor: value.cursor,
            data: value.data,
            next_cursor: value.nextCursor ?? null,
            total_count: value.totalCount,
            digest: value.digest,
          })),
        ),
    )
    .catch((error: unknown) =>
      err(error instanceof Error ? error : new Error(String(error))),
    )
}

export const resolveLlmPhaseInputs = async (
  input: Readonly<Record<string, InputBinding>> | undefined,
  priorResults: Readonly<Record<string, unknown>>,
  context: RunAgentGraphContext,
): Promise<Result<Readonly<Record<string, unknown>>, unknown>> => {
  const resolved: Record<string, unknown> = {}
  for (const [field, binding] of Object.entries(input ?? {})) {
    if ('literal' in binding) {
      resolved[field] = binding.literal
      continue
    }
    const value = await resolvePayloadBinding(binding, priorResults, context)
    if (value.isErr()) return err(value.error)
    resolved[field] = value.value
  }
  return ok(resolved)
}
