import { captureWithFingerprint } from '@fohte/service-kit/observability'
import { createMiddleware } from 'langchain'

import { bindModelCallSignal } from '#strategy-agent/bind-model-call-signal'

// signal で中断されるモデル呼び出し middleware の共通実装。abort 時に警告ログを出し、
// Sentry にエラーとして記録した上で、その旨を表すメッセージへ差し替えて再送出する。
// getSignal は呼び出しごとに呼ばれるため、呼び出しのたびに新しい signal を生成する
// factory (例: () => AbortSignal.timeout(ms)) を渡せる。
export const createAbortingModelCallMiddleware = (
  name: string,
  getSignal: () => AbortSignal,
  fingerprint: string,
  buildErrorMessage: () => string,
) =>
  createMiddleware({
    name,
    wrapModelCall: (request, handler) => {
      const signal = getSignal()
      const model = bindModelCallSignal(request.model, { signal })
      return Promise.resolve(handler({ ...request, model })).catch(
        (error: unknown) => {
          // eslint-disable-next-line no-restricted-syntax -- abort によるものでないエラーはそのまま呼び出し元に伝播させる必要がある
          if (!signal.aborted) throw error
          const message = buildErrorMessage()
          console.warn(message)
          captureWithFingerprint(new Error(message), fingerprint)
          // eslint-disable-next-line no-restricted-syntax -- abort 起因のエラーを人間可読なメッセージに変換して再送出する。呼び出し元 (invokePhaseWithRetry 等) は reject をそのまま Result に変換する契約のため、ここで catch した後は再送出するしかない
          throw new Error(message)
        },
      )
    },
  })
