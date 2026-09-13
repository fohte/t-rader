import { captureWithFingerprint } from '@fohte/service-kit/observability'
import { createMiddleware } from 'langchain'

import { bindModelCallSignal } from '#strategy-agent/bind-model-call-signal'

// signal で中断されるモデル呼び出し middleware の共通実装。abort 時に警告ログと
// メトリクスを記録する。
export const createAbortingModelCallMiddleware = (
  name: string,
  signal: AbortSignal,
  fingerprint: string,
  buildErrorMessage: () => string,
) =>
  createMiddleware({
    name,
    wrapModelCall: (request, handler) => {
      const model = bindModelCallSignal(request.model, { signal })
      return Promise.resolve(handler({ ...request, model })).finally(() => {
        if (!signal.aborted) return
        const error = new Error(buildErrorMessage())
        console.warn(error.message)
        captureWithFingerprint(error, fingerprint)
      })
    },
  })
