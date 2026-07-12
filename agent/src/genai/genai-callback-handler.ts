import { BaseCallbackHandler } from '@langchain/core/callbacks/base'
import type { Serialized } from '@langchain/core/load/serializable'
import type { BaseMessage } from '@langchain/core/messages'
import type { ChatGeneration, LLMResult } from '@langchain/core/outputs'
import { type Span, SpanKind, SpanStatusCode, trace } from '@opentelemetry/api'
import {
  ATTR_GEN_AI_INPUT_MESSAGES,
  ATTR_GEN_AI_OPERATION_NAME,
  ATTR_GEN_AI_OUTPUT_MESSAGES,
  ATTR_GEN_AI_PROVIDER_NAME,
  ATTR_GEN_AI_REQUEST_MODEL,
  ATTR_GEN_AI_RESPONSE_FINISH_REASONS,
  ATTR_GEN_AI_RESPONSE_MODEL,
  ATTR_GEN_AI_USAGE_INPUT_TOKENS,
  ATTR_GEN_AI_USAGE_OUTPUT_TOKENS,
  GEN_AI_OPERATION_NAME_VALUE_CHAT,
} from '@opentelemetry/semantic-conventions/incubating'

// Mirrors the env var used by other OpenTelemetry GenAI instrumentations
// (e.g. opentelemetry-instrumentation-openai-v2, Elastic's EDOT Node.js SDK)
// to gate capture of message content, which is opt-in per the GenAI semantic
// conventions because it may contain PII.
const CAPTURE_MESSAGE_CONTENT_ENV_VAR =
  'OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT'

const tracer = trace.getTracer('t-rader-agent-genai')

export interface GenAiCallbackHandlerOptions {
  readonly providerName: string
  readonly captureMessageContent?: boolean
  readonly env?: Readonly<Record<string, string | undefined>>
}

interface GenAiMessagePart {
  readonly type: 'text'
  readonly content: string
}

interface GenAiMessage {
  readonly role: string
  readonly parts: readonly GenAiMessagePart[]
}

const contentToText = (content: BaseMessage['content']): string =>
  typeof content === 'string' ? content : JSON.stringify(content)

const messageToGenAiMessage = (message: BaseMessage): GenAiMessage => ({
  role: message.type,
  parts: [{ type: 'text', content: contentToText(message.content) }],
})

const recordSpanException = (span: Span, error: unknown): void => {
  span.recordException(error instanceof Error ? error : String(error))
  span.setStatus({
    code: SpanStatusCode.ERROR,
    message: error instanceof Error ? error.message : String(error),
  })
}

// LangChain's ChatOpenAI (and providers following its conventions) attach
// these to LLMResult without a shared exported type: `llmOutput.tokenUsage`
// and per-generation `generationInfo.model_name` / `generationInfo.finish_reason`.
interface TokenUsage {
  readonly promptTokens?: number
  readonly completionTokens?: number
}

// One CLIENT span per model inference call, matching the GenAI semantic
// conventions' `{gen_ai.operation.name} {gen_ai.request.model}` span name.
export class GenAiCallbackHandler extends BaseCallbackHandler {
  name = 'GenAiCallbackHandler'

  private readonly providerName: string
  private readonly captureMessageContent: boolean
  private readonly spans = new Map<string, Span>()

  constructor(options: GenAiCallbackHandlerOptions) {
    super()
    this.providerName = options.providerName
    this.captureMessageContent =
      options.captureMessageContent ??
      (options.env ?? process.env)[CAPTURE_MESSAGE_CONTENT_ENV_VAR] === 'true'
  }

  override handleChatModelStart(
    _llm: Serialized,
    messages: BaseMessage[][],
    runId: string,
    _parentRunId?: string,
    extraParams?: Record<string, unknown>,
  ): void {
    // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- extraParams is an untyped bag; narrowed immediately below via typeof
    const invocationParams = extraParams?.['invocation_params'] as
      Record<string, unknown> | undefined
    const model =
      typeof invocationParams?.['model'] === 'string'
        ? invocationParams['model']
        : 'unknown'

    const span = tracer.startSpan(
      `${GEN_AI_OPERATION_NAME_VALUE_CHAT} ${model}`,
      {
        kind: SpanKind.CLIENT,
      },
    )
    span.setAttributes({
      [ATTR_GEN_AI_OPERATION_NAME]: GEN_AI_OPERATION_NAME_VALUE_CHAT,
      [ATTR_GEN_AI_PROVIDER_NAME]: this.providerName,
      [ATTR_GEN_AI_REQUEST_MODEL]: model,
    })
    if (this.captureMessageContent) {
      try {
        span.setAttribute(
          ATTR_GEN_AI_INPUT_MESSAGES,
          JSON.stringify(messages.flat().map(messageToGenAiMessage)),
        )
      } catch (error) {
        recordSpanException(span, error)
      }
    }
    this.spans.set(runId, span)
  }

  override handleLLMEnd(output: LLMResult, runId: string): void {
    const span = this.spans.get(runId)
    if (span === undefined) return
    this.spans.delete(runId)

    try {
      const generation = output.generations[0]?.[0]
      const generationInfo = generation?.generationInfo
      const modelName: unknown = generationInfo?.['model_name']
      if (typeof modelName === 'string') {
        span.setAttribute(ATTR_GEN_AI_RESPONSE_MODEL, modelName)
      }
      const finishReason: unknown = generationInfo?.['finish_reason']
      if (typeof finishReason === 'string') {
        span.setAttribute(ATTR_GEN_AI_RESPONSE_FINISH_REASONS, [finishReason])
      }
      // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- llmOutput is an untyped bag; narrowed immediately below via typeof
      const tokenUsage = output.llmOutput?.['tokenUsage'] as
        TokenUsage | undefined
      if (typeof tokenUsage?.promptTokens === 'number') {
        span.setAttribute(
          ATTR_GEN_AI_USAGE_INPUT_TOKENS,
          tokenUsage.promptTokens,
        )
      }
      if (typeof tokenUsage?.completionTokens === 'number') {
        span.setAttribute(
          ATTR_GEN_AI_USAGE_OUTPUT_TOKENS,
          tokenUsage.completionTokens,
        )
      }
      if (
        this.captureMessageContent &&
        generation !== undefined &&
        'message' in generation
      ) {
        // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- the `'message' in generation` check above confirms this is a ChatGeneration, but TS cannot narrow a non-discriminated interface
        const chatGeneration = generation as ChatGeneration
        span.setAttribute(
          ATTR_GEN_AI_OUTPUT_MESSAGES,
          JSON.stringify([messageToGenAiMessage(chatGeneration.message)]),
        )
      }
    } catch (error) {
      recordSpanException(span, error)
    } finally {
      span.end()
    }
  }

  override handleLLMError(err: unknown, runId: string): void {
    const span = this.spans.get(runId)
    if (span === undefined) return
    this.spans.delete(runId)

    recordSpanException(span, err)
    span.end()
  }
}
