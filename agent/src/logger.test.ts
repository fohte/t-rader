import { AsyncLocalStorage } from 'node:async_hooks'

import {
  type Context,
  context,
  type ContextManager,
  ROOT_CONTEXT,
  trace,
} from '@opentelemetry/api'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { z } from 'zod'

import { createAppLogger, withLogBindings } from '#logger'

// SDK を入れずに context.with() で span を伝播させるための最小の context manager。
class AsyncLocalStorageContextManager implements ContextManager {
  private readonly storage = new AsyncLocalStorage<Context>()

  active(): Context {
    return this.storage.getStore() ?? ROOT_CONTEXT
  }

  with<A extends unknown[], F extends (...args: A) => ReturnType<F>>(
    ctx: Context,
    fn: F,
    thisArg?: ThisParameterType<F>,
    ...args: A
  ): ReturnType<F> {
    return this.storage.run(ctx, () => fn.call(thisArg, ...args))
  }

  bind<T>(_ctx: Context, target: T): T {
    return target
  }

  enable(): this {
    return this
  }

  disable(): this {
    this.storage.disable()
    return this
  }
}

const TRACE_ID = '0af7651916cd43dd8448eb211c80319c'
const SPAN_ID = 'b7ad6b7169203331'

const withActiveSpan = <T>(
  spanContext: { traceId: string; spanId: string },
  fn: () => T,
): T => {
  const span = trace.wrapSpanContext({ ...spanContext, traceFlags: 1 })
  return context.with(trace.setSpan(context.active(), span), fn)
}

const setup = () => {
  const lines: string[] = []
  const logger = createAppLogger({
    destination: { write: (line: string) => void lines.push(line) },
  })
  // time は実行ごとに変わるため、固定値に置き換えて全体を比較できるようにする。
  const output = () =>
    lines.map((line) => ({
      ...z.record(z.string(), z.unknown()).parse(JSON.parse(line)),
      time: '<time>',
    }))
  return { logger, output }
}

describe('logger', () => {
  const contextManager = new AsyncLocalStorageContextManager()
  beforeAll(() => {
    context.setGlobalContextManager(contextManager)
  })
  afterAll(() => {
    context.disable()
  })

  it('adds trace_id and span_id of the active span', () => {
    const { logger, output } = setup()

    withActiveSpan({ traceId: TRACE_ID, spanId: SPAN_ID }, () => {
      logger.info({ foo: 'bar' }, 'hello')
    })

    expect(output()).toEqual([
      {
        level: 30,
        time: '<time>',
        trace_id: TRACE_ID,
        span_id: SPAN_ID,
        foo: 'bar',
        msg: 'hello',
      },
    ])
  })

  it.each([
    {
      name: 'there is no active span',
      run: (log: () => void) => {
        log()
      },
    },
    {
      // OTel 未設定時に NoopTracer が返す無効な span context。
      name: 'the active span context is invalid',
      run: (log: () => void) => {
        withActiveSpan({ traceId: '0'.repeat(32), spanId: '0'.repeat(16) }, log)
      },
    },
  ])('omits trace_id and span_id when $name', ({ run }) => {
    const { logger, output } = setup()

    run(() => {
      logger.info({}, 'hello')
    })

    expect(output()).toEqual([{ level: 30, time: '<time>', msg: 'hello' }])
  })

  it('adds withLogBindings bindings to log lines emitted after an await', async () => {
    const { logger, output } = setup()

    await withLogBindings({ task_id: 't1' }, async () => {
      await Promise.resolve()
      logger.info({}, 'hello')
    })

    expect(output()).toEqual([
      { level: 30, time: '<time>', task_id: 't1', msg: 'hello' },
    ])
  })

  it('merges nested withLogBindings scopes on top of the outer bindings', () => {
    const { logger, output } = setup()

    withLogBindings({ task_id: 't1' }, () => {
      withLogBindings({ 'phase.key': 'plan' }, () => {
        logger.info({}, 'hello')
      })
    })

    expect(output()).toEqual([
      {
        level: 30,
        time: '<time>',
        task_id: 't1',
        'phase.key': 'plan',
        msg: 'hello',
      },
    ])
  })

  it('does not add withLogBindings bindings to log lines emitted outside the scope', () => {
    const { logger, output } = setup()

    withLogBindings({ task_id: 't1' }, () => undefined)
    logger.info({}, 'hello')

    expect(output()).toEqual([{ level: 30, time: '<time>', msg: 'hello' }])
  })

  it('adds the context fields to child loggers as well', () => {
    const { logger, output } = setup()

    withActiveSpan({ traceId: TRACE_ID, spanId: SPAN_ID }, () => {
      logger.child({ component: 'c' }).warn({}, 'hello')
    })

    expect(output()).toEqual([
      {
        level: 40,
        time: '<time>',
        component: 'c',
        trace_id: TRACE_ID,
        span_id: SPAN_ID,
        msg: 'hello',
      },
    ])
  })

  it('redacts secret fields', () => {
    const { logger, output } = setup()

    logger.info({ api_key: 'secret', nested: { token: 'secret' } }, 'hello')

    expect(output()).toEqual([
      {
        level: 30,
        time: '<time>',
        api_key: '[REDACTED]',
        nested: { token: '[REDACTED]' },
        msg: 'hello',
      },
    ])
  })
})
