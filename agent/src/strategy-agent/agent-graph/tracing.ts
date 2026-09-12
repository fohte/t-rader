import { context, SpanStatusCode, trace } from '@opentelemetry/api'
import {
  ATTR_GEN_AI_OPERATION_NAME,
  GEN_AI_OPERATION_NAME_VALUE_INVOKE_AGENT,
} from '@opentelemetry/semantic-conventions/incubating'
import type { Result } from 'neverthrow'

const TRACER_NAME = 't-rader-agent-graph'

// @fohte/service-kit の genai-tracing-middleware と同じ方式: ここで開始し
// context.with() で入った span は、`fn` 内でそのミドルウェアが生成する
// chat/execute_tool span の暗黙の親になる。そのためフェーズ (と for_each の
// 各要素) は、自身のモデル呼び出しの上位に span としてトレースツリーに現れる。
export interface PhaseSpanIds {
  readonly traceId: string
  readonly spanId: string
}

export const withPhaseSpan = async <T, E>(
  name: string,
  attributes: Record<string, string | number>,
  fn: (spanIds: PhaseSpanIds) => Promise<Result<T, E>>,
): Promise<Result<T, E>> => {
  const tracer = trace.getTracer(TRACER_NAME)
  // Langfuse は gen_ai.* 属性を持たない span を observation として取り込まない
  // (LLM 関連の span のみ通すフィルタを持つため)。invoke_agent はエージェント
  // 呼び出しループを表す正規の gen_ai.operation.name 値。
  const span = tracer.startSpan(name, {
    attributes: {
      ...attributes,
      [ATTR_GEN_AI_OPERATION_NAME]: GEN_AI_OPERATION_NAME_VALUE_INVOKE_AGENT,
    },
  })
  const { traceId, spanId } = span.spanContext()
  const spanContext = trace.setSpan(context.active(), span)

  // eslint-disable-next-line no-restricted-syntax -- span.end() を finally で必ず呼ぶため try/finally が必要
  try {
    const result = await context.with(spanContext, () =>
      fn({ traceId, spanId }),
    )
    if (result.isErr()) {
      const error = result.error
      span.recordException(
        error instanceof Error ? error : new Error(String(error)),
      )
      span.setStatus({ code: SpanStatusCode.ERROR })
    }
    return result
  } finally {
    span.end()
  }
}
