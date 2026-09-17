import { captureWithFingerprint } from '@fohte/service-kit/observability'
import type {
  HandleLLMNewTokenCallbackFields,
  NewTokenIndices,
} from '@langchain/core/callbacks/base'
import { BaseCallbackHandler } from '@langchain/core/callbacks/base'
import { AIMessageChunk } from '@langchain/core/messages'
import { createMiddleware } from 'langchain'

import { bindModelCallSignal } from '#strategy-agent/bind-model-call-signal'

export const MAX_TOOL_CALLS_PER_MODEL_CALL = 50

const TOOL_CALL_CAP_EXCEEDED_FINGERPRINT = 'tool-call-cap-middleware.exceeded'

// ChatOpenAI (streaming: true) は tool_call_chunks を index ごとの断片で
// SSE delta 単位に流すため、同じ index の重複を除いた distinct 数で数える。
class ToolCallCountingHandler extends BaseCallbackHandler {
  name = 'toolCallCapMiddleware.counter'

  // name は通常その index の最初の断片にだけ入るため、来た時点で保持しておく。
  private readonly toolNameByIndex = new Map<number, string>()

  constructor(
    private readonly maxToolCalls: number,
    private readonly abortController: AbortController,
  ) {
    super()
  }

  override handleLLMNewToken(
    _token: string,
    _idx: NewTokenIndices,
    _runId: string,
    _parentRunId: string | undefined,
    _tags: string[] | undefined,
    fields?: HandleLLMNewTokenCallbackFields,
  ): void {
    const chunk = fields?.chunk
    const message =
      chunk !== undefined && 'message' in chunk ? chunk.message : undefined
    if (!AIMessageChunk.isInstance(message)) return

    for (const toolCallChunk of message.tool_call_chunks ?? []) {
      if (toolCallChunk.index === undefined) continue
      if (toolCallChunk.name !== undefined) {
        this.toolNameByIndex.set(toolCallChunk.index, toolCallChunk.name)
      } else if (!this.toolNameByIndex.has(toolCallChunk.index)) {
        this.toolNameByIndex.set(toolCallChunk.index, 'unknown')
      }
    }
    if (this.toolNameByIndex.size > this.maxToolCalls) {
      this.abortController.abort()
    }
  }

  toolCallCountsByName(): Record<string, number> {
    const counts: Record<string, number> = {}
    for (const name of this.toolNameByIndex.values()) {
      counts[name] = (counts[name] ?? 0) + 1
    }
    return counts
  }
}

const formatToolCallCounts = (counts: Record<string, number>): string =>
  Object.entries(counts)
    .sort(([aName, aCount], [bName, bCount]) =>
      aCount !== bCount ? bCount - aCount : aName.localeCompare(bName),
    )
    .map(([name, count]) => `${name}: ${String(count)}`)
    .join(', ')

export const createToolCallCapMiddleware = (maxToolCalls: number) =>
  createMiddleware({
    name: 'toolCallCapMiddleware',
    wrapModelCall: (request, handler) => {
      const abortController = new AbortController()
      const counter = new ToolCallCountingHandler(maxToolCalls, abortController)
      const model = bindModelCallSignal(request.model, {
        signal: abortController.signal,
        callbacks: [counter],
      })

      return Promise.resolve(handler({ ...request, model })).finally(() => {
        if (!abortController.signal.aborted) return
        const toolCallCountsByName = counter.toolCallCountsByName()
        const error = new Error(
          `toolCallCapMiddleware: aborted model call after exceeding ${String(maxToolCalls)} tool call(s) in a single response (${formatToolCallCounts(toolCallCountsByName)})`,
        )
        console.warn(error.message)
        captureWithFingerprint(error, TOOL_CALL_CAP_EXCEEDED_FINGERPRINT, {
          extras: { toolCallCountsByName },
        })
      })
    },
  })
