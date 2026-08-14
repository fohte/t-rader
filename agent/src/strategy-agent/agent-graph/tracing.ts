import { context, SpanStatusCode, trace } from '@opentelemetry/api'
import type { Result } from 'neverthrow'

const TRACER_NAME = 't-rader-agent-graph'

// Mirrors @fohte/service-kit's genai-tracing-middleware: a span started here
// and entered via context.with() becomes the ambient parent for whatever
// chat/execute_tool spans that middleware creates inside `fn`, so a phase
// (and each for_each item) shows up as a span in the trace tree above its
// model calls.
export const withPhaseSpan = async <T, E>(
  name: string,
  attributes: Record<string, string | number>,
  fn: () => Promise<Result<T, E>>,
): Promise<Result<T, E>> => {
  const tracer = trace.getTracer(TRACER_NAME)
  const span = tracer.startSpan(name, { attributes })
  const spanContext = trace.setSpan(context.active(), span)

  const result = await context.with(spanContext, fn)
  if (result.isErr()) {
    const error = result.error
    span.recordException(
      error instanceof Error ? error : new Error(String(error)),
    )
    span.setStatus({ code: SpanStatusCode.ERROR })
  }
  span.end()
  return result
}
