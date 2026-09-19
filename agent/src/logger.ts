import { AsyncLocalStorage } from 'node:async_hooks'

import {
  createLogger,
  type CreateLoggerOptions,
  type LogFields,
  type Logger,
} from '@fohte/service-kit/logger'
import { isSpanContextValid, trace } from '@opentelemetry/api'

export type { Logger }

// OTel の context ではなく AsyncLocalStorage を使うのは、OTel が未設定
// (context manager が noop) の環境でも task_id 等を全ログ行に付けるため。
const bindingsStorage = new AsyncLocalStorage<LogFields>()

// fn の同期/非同期の実行全体で、logger が出す全ログ行に bindings を付与する。
export const withLogBindings = <T>(bindings: LogFields, fn: () => T): T =>
  bindingsStorage.run({ ...bindingsStorage.getStore(), ...bindings }, fn)

// service-kit の createLogger は pino の mixin を受け付けないため、呼び出しごとに
// アクティブな span から trace_id / span_id を取り出して fields にマージする。
// span が無い (起動・停止時) か、OTel 未設定で NoopTracer が返す無効な
// span context の場合は trace_id / span_id を付けない。
const contextFields = (): LogFields => {
  const spanContext = trace.getActiveSpan()?.spanContext()
  return {
    ...bindingsStorage.getStore(),
    ...(spanContext !== undefined && isSpanContextValid(spanContext)
      ? { trace_id: spanContext.traceId, span_id: spanContext.spanId }
      : {}),
  }
}

const withContextFields = (base: Logger): Logger => {
  const wrap =
    (level: 'trace' | 'debug' | 'info' | 'warn' | 'error' | 'fatal') =>
    (fields: LogFields, message: string): void => {
      base[level]({ ...contextFields(), ...fields }, message)
    }
  return {
    trace: wrap('trace'),
    debug: wrap('debug'),
    info: wrap('info'),
    warn: wrap('warn'),
    error: wrap('error'),
    fatal: wrap('fatal'),
    child: (bindings) => withContextFields(base.child(bindings)),
  }
}

export const createAppLogger = (options?: CreateLoggerOptions): Logger =>
  withContextFields(createLogger(options))

export const logger = createAppLogger()
