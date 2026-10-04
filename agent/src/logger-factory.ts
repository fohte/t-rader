import { AsyncLocalStorage } from 'node:async_hooks'

import {
  createLogger,
  type CreateLoggerOptions,
  type LogFields,
  type Logger,
} from '@fohte/service-kit/logger'
import { isSpanContextValid, trace } from '@opentelemetry/api'

export type { Logger }

const bindingsStorage = new AsyncLocalStorage<LogFields>()

export const withLogBindings = <T>(bindings: LogFields, fn: () => T): T =>
  bindingsStorage.run({ ...bindingsStorage.getStore(), ...bindings }, fn)

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
