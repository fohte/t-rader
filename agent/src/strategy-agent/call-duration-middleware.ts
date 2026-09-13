import { createAbortingModelCallMiddleware } from '#strategy-agent/aborting-model-call-middleware'

const CALL_DURATION_TIMEOUT_FINGERPRINT = 'call-duration-middleware.timeout'

// ChatOpenAI の timeout はヘッダ受信までしか縛らず、streaming 中の暴走を止められない。
// 呼び出しごとに RunnableBinding 経由で AbortSignal を注入し、AgentNode 側の config
// マージで signal が上書き消失しないようにする。
export const createCallDurationMiddleware = (timeoutMs: number) =>
  createAbortingModelCallMiddleware(
    'callDurationMiddleware',
    AbortSignal.timeout(timeoutMs),
    CALL_DURATION_TIMEOUT_FINGERPRINT,
    () =>
      `callDurationMiddleware: aborted model call after exceeding ${String(timeoutMs)}ms`,
  )
