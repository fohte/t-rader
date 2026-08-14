import { context, SpanStatusCode, trace } from '@opentelemetry/api'
import type { Result } from 'neverthrow'

const TRACER_NAME = 't-rader-agent-graph'

// @fohte/service-kit の genai-tracing-middleware と同じ方式: ここで開始し
// context.with() で入った span は、`fn` 内でそのミドルウェアが生成する
// chat/execute_tool span の暗黙の親になる。そのためフェーズ (と for_each の
// 各要素) は、自身のモデル呼び出しの上位に span としてトレースツリーに現れる。
export const withPhaseSpan = async <T, E>(
  name: string,
  attributes: Record<string, string | number>,
  fn: () => Promise<Result<T, E>>,
): Promise<Result<T, E>> => {
  const tracer = trace.getTracer(TRACER_NAME)
  const span = tracer.startSpan(name, { attributes })
  const spanContext = trace.setSpan(context.active(), span)

  // eslint-disable-next-line no-restricted-syntax -- span.end() を finally で必ず呼ぶため try/finally が必要
  try {
    const result = await context.with(spanContext, fn)
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
