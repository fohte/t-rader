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

  private readonly seenIndices = new Set<number>()

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
      if (toolCallChunk.index !== undefined) {
        this.seenIndices.add(toolCallChunk.index)
      }
    }
    if (this.seenIndices.size > this.maxToolCalls) this.abortController.abort()
  }
}

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
        const error = new Error(
          `toolCallCapMiddleware: aborted model call after exceeding ${String(maxToolCalls)} tool call(s) in a single response`,
        )
        console.warn(error.message)
        captureWithFingerprint(error, TOOL_CALL_CAP_EXCEEDED_FINGERPRINT)
      })
    },
  })
