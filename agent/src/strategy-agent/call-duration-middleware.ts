import { captureWithFingerprint } from '@fohte/service-kit/observability'
import { Runnable, RunnableBinding } from '@langchain/core/runnables'
import { createMiddleware } from 'langchain'

const CALL_DURATION_TIMEOUT_FINGERPRINT = 'call-duration-middleware.timeout'

// ChatOpenAI の timeout はヘッダ受信までしか縛らず、streaming 中の暴走を止められない。
// 呼び出しごとに RunnableBinding 経由で AbortSignal を注入し、AgentNode 側の config
// マージで signal が上書き消失しないようにする。
export const createCallDurationMiddleware = (timeoutMs: number) =>
  createMiddleware({
    name: 'callDurationMiddleware',
    wrapModelCall: (request, handler) => {
      // request.model の型 (AgentLanguageModelLike) は RunnableBinding.bound が
      // 要求する具象 Runnable より緩いため、実行時に確認できない場合は素通しする。
      if (!(request.model instanceof Runnable)) return handler(request)

      const signal = AbortSignal.timeout(timeoutMs)
      return Promise.resolve(
        handler({
          ...request,
          model: new RunnableBinding({
            bound: request.model,
            config: { signal },
            kwargs: {},
          }),
        }),
      ).finally(() => {
        if (!signal.aborted) return
        const error = new Error(
          `callDurationMiddleware: aborted model call after exceeding ${String(timeoutMs)}ms`,
        )
        console.warn(error.message)
        captureWithFingerprint(error, CALL_DURATION_TIMEOUT_FINGERPRINT)
      })
    },
  })
