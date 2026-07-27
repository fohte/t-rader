import {
  AIMessage,
  HumanMessage,
  SystemMessage,
} from '@langchain/core/messages'
import type { ChatGeneration, LLMResult } from '@langchain/core/outputs'
import { SpanKind, SpanStatusCode, trace } from '@opentelemetry/api'
import {
  BasicTracerProvider,
  InMemorySpanExporter,
  SimpleSpanProcessor,
} from '@opentelemetry/sdk-trace-base'
import { ATTR_EXCEPTION_TYPE } from '@opentelemetry/semantic-conventions'
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
} from '@opentelemetry/semantic-conventions/incubating'
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
} from 'vitest'

import { GenAiCallbackHandler } from '#genai/genai-callback-handler'

const exporter = new InMemorySpanExporter()
const provider = new BasicTracerProvider({
  spanProcessors: [new SimpleSpanProcessor(exporter)],
})

beforeAll(() => {
  trace.setGlobalTracerProvider(provider)
})

afterAll(async () => {
  trace.disable()
  await provider.shutdown()
})

beforeEach(() => {
  exporter.reset()
})

const buildLlmResult = (overrides: {
  modelName?: string
  finishReason?: string
  content?: string
  tokenUsage?: { promptTokens: number; completionTokens: number }
}): LLMResult => {
  const generation: ChatGeneration = {
    text: overrides.content ?? 'hi',
    message: new AIMessage(overrides.content ?? 'hi'),
    generationInfo: {
      ...(overrides.modelName !== undefined
        ? { model_name: overrides.modelName }
        : {}),
      ...(overrides.finishReason !== undefined
        ? { finish_reason: overrides.finishReason }
        : {}),
    },
  }
  return {
    generations: [[generation]],
    ...(overrides.tokenUsage !== undefined
      ? { llmOutput: { tokenUsage: overrides.tokenUsage } }
      : {}),
  }
}

describe('GenAiCallbackHandler', () => {
  afterEach(() => {
    exporter.reset()
  })

  it('records gen_ai attributes and token usage across handleChatModelStart/handleLLMEnd', () => {
    const handler = new GenAiCallbackHandler({
      providerName: 'opencode',
      captureMessageContent: false,
    })

    handler.handleChatModelStart(
      { lc: 1, type: 'not_implemented', id: [] },
      [[new SystemMessage('be terse'), new HumanMessage('hi')]],
      'run-1',
      undefined,
      { invocation_params: { model: 'test-model' } },
    )
    handler.handleLLMEnd(
      buildLlmResult({
        modelName: 'resolved-model',
        finishReason: 'stop',
        tokenUsage: { promptTokens: 42, completionTokens: 7 },
      }),
      'run-1',
    )

    const spans = exporter.getFinishedSpans()
    expect(
      spans.map((span) => ({
        name: span.name,
        kind: span.kind,
        status: span.status,
        attributes: span.attributes,
      })),
    ).toEqual([
      {
        name: 'chat test-model',
        kind: SpanKind.CLIENT,
        status: { code: SpanStatusCode.UNSET },
        attributes: {
          [ATTR_GEN_AI_OPERATION_NAME]: 'chat',
          [ATTR_GEN_AI_PROVIDER_NAME]: 'opencode',
          [ATTR_GEN_AI_REQUEST_MODEL]: 'test-model',
          [ATTR_GEN_AI_RESPONSE_MODEL]: 'resolved-model',
          [ATTR_GEN_AI_USAGE_INPUT_TOKENS]: 42,
          [ATTR_GEN_AI_USAGE_OUTPUT_TOKENS]: 7,
          [ATTR_GEN_AI_RESPONSE_FINISH_REASONS]: ['stop'],
        },
      },
    ])
  })

  it('does not record gen_ai.input.messages / gen_ai.output.messages unless captureMessageContent is enabled', () => {
    const handler = new GenAiCallbackHandler({
      providerName: 'opencode',
      captureMessageContent: false,
    })

    handler.handleChatModelStart(
      { lc: 1, type: 'not_implemented', id: [] },
      [[new HumanMessage('hi')]],
      'run-2',
      undefined,
      { invocation_params: { model: 'test-model' } },
    )
    handler.handleLLMEnd(buildLlmResult({ content: 'hello' }), 'run-2')

    const [span] = exporter.getFinishedSpans()
    expect(span?.attributes[ATTR_GEN_AI_INPUT_MESSAGES]).toBeUndefined()
    expect(span?.attributes[ATTR_GEN_AI_OUTPUT_MESSAGES]).toBeUndefined()
  })

  it('captures gen_ai.input.messages / gen_ai.output.messages when captureMessageContent is enabled', () => {
    const handler = new GenAiCallbackHandler({
      providerName: 'opencode',
      captureMessageContent: true,
    })

    handler.handleChatModelStart(
      { lc: 1, type: 'not_implemented', id: [] },
      [[new SystemMessage('be terse'), new HumanMessage('hi')]],
      'run-3',
      undefined,
      { invocation_params: { model: 'test-model' } },
    )
    handler.handleLLMEnd(buildLlmResult({ content: 'hello there' }), 'run-3')

    const [span] = exporter.getFinishedSpans()
    expect(
      JSON.parse(String(span?.attributes[ATTR_GEN_AI_INPUT_MESSAGES])),
    ).toEqual([
      { role: 'system', parts: [{ type: 'text', content: 'be terse' }] },
      { role: 'human', parts: [{ type: 'text', content: 'hi' }] },
    ])
    expect(
      JSON.parse(String(span?.attributes[ATTR_GEN_AI_OUTPUT_MESSAGES])),
    ).toEqual([
      { role: 'ai', parts: [{ type: 'text', content: 'hello there' }] },
    ])
  })

  it('falls back to the OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT env var when captureMessageContent is not set', () => {
    const handler = new GenAiCallbackHandler({
      providerName: 'opencode',
      env: { OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT: 'true' },
    })

    handler.handleChatModelStart(
      { lc: 1, type: 'not_implemented', id: [] },
      [[new HumanMessage('hi')]],
      'run-4',
      undefined,
      { invocation_params: { model: 'test-model' } },
    )
    handler.handleLLMEnd(buildLlmResult({ content: 'hello' }), 'run-4')

    const [span] = exporter.getFinishedSpans()
    expect(span?.attributes[ATTR_GEN_AI_INPUT_MESSAGES] !== undefined).toBe(
      true,
    )
  })

  it('records handleLLMError as a span exception and marks the span ERROR', () => {
    const handler = new GenAiCallbackHandler({ providerName: 'opencode' })

    handler.handleChatModelStart(
      { lc: 1, type: 'not_implemented', id: [] },
      [[new HumanMessage('hi')]],
      'run-5',
      undefined,
      { invocation_params: { model: 'test-model' } },
    )
    handler.handleLLMError(new Error('boom'), 'run-5')

    const spans = exporter.getFinishedSpans()
    expect(
      spans.map((span) => ({
        name: span.name,
        status: span.status,
        exceptionTypes: span.events
          .filter((e) => e.name === 'exception')
          .map((e) => e.attributes?.[ATTR_EXCEPTION_TYPE]),
      })),
    ).toEqual([
      {
        name: 'chat test-model',
        status: { code: SpanStatusCode.ERROR, message: 'boom' },
        exceptionTypes: ['Error'],
      },
    ])
  })

  it('ignores handleLLMEnd/handleLLMError for an unknown runId', () => {
    const handler = new GenAiCallbackHandler({ providerName: 'opencode' })

    handler.handleLLMEnd(buildLlmResult({}), 'unknown-run')
    handler.handleLLMError(new Error('boom'), 'unknown-run')

    expect(exporter.getFinishedSpans()).toEqual([])
  })
})
