import { captureWithFingerprint } from '@fohte/service-kit/observability'
import type {
  HandleLLMNewTokenCallbackFields,
  NewTokenIndices,
} from '@langchain/core/callbacks/base'
import { BaseCallbackHandler } from '@langchain/core/callbacks/base'
import { AIMessageChunk } from '@langchain/core/messages'
import {
  mergeConfigs,
  Runnable,
  RunnableBinding,
} from '@langchain/core/runnables'
import { createMiddleware } from 'langchain'

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

// 応答完了後に tool_calls.length を見る方式 (model-response-guard-middleware
// が完了後の response.tool_calls を検査するのと同じ位置) では、暴走応答の
// 生成待ち時間そのものは縮まらない。streaming 中に distinct tool call index
// を数え、上限を超えた時点で打ち切ることで生成待ちごと止める。
// call-duration-middleware と同じく RunnableBinding 経由で signal を注入する
// (AgentNode の config マージで signal が上書き消失しないようにするため)。
export const createToolCallCapMiddleware = (maxToolCalls: number) =>
  createMiddleware({
    name: 'toolCallCapMiddleware',
    wrapModelCall: (request, handler) => {
      // request.model の型 (AgentLanguageModelLike) は RunnableBinding.bound
      // が要求する具象 Runnable より緩いため、実行時に確認できない場合は
      // 素通しする。
      if (!(request.model instanceof Runnable)) return handler(request)

      const abortController = new AbortController()
      const counter = new ToolCallCountingHandler(maxToolCalls, abortController)
      const addedConfig = {
        signal: abortController.signal,
        callbacks: [counter],
      }
      // createAgent の bindTools 解決 (langchain の _simpleBindTools) は
      // RunnableBinding を 1 段しか unwrap しない。call-duration-middleware も
      // 同じ手段で signal を注入するため、既に RunnableBinding ならその
      // config に merge し、二重にラップして bindTools 解決を壊さないようにする。
      const model = RunnableBinding.isRunnableBinding(request.model)
        ? new RunnableBinding({
            bound: request.model.bound,
            config: mergeConfigs(request.model.config, addedConfig),
            kwargs: request.model.kwargs ?? {},
          })
        : new RunnableBinding({
            bound: request.model,
            config: addedConfig,
            kwargs: {},
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
