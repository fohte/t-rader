import { captureWithFingerprint } from '@fohte/service-kit/observability'
import { createMiddleware } from 'langchain'
import { z } from 'zod'

const FINAL_TURN_FORCED_FINGERPRINT = 'final-turn-middleware.forced-submission'

// 通常 tool の呼び出し上限。到達ターンでは提出用 tool 以外を外して構造化出力を強制する。
export const MAX_MODEL_CALLS_PER_INVOKE = 15

export const finalTurnMiddleware = createMiddleware({
  name: 'finalTurnMiddleware',
  stateSchema: z.object({
    modelCallCount: z.number().default(0),
  }),
  beforeModel: (state) => ({ modelCallCount: state.modelCallCount + 1 }),
  wrapModelCall: (request, handler) => {
    if (request.state.modelCallCount < MAX_MODEL_CALLS_PER_INVOKE) {
      return handler(request)
    }
    const error = new Error(
      `finalTurnMiddleware: forcing structured-output submission at model call ${String(request.state.modelCallCount)}`,
    )
    console.warn(error.message)
    captureWithFingerprint(error, FINAL_TURN_FORCED_FINGERPRINT)
    // 構造化出力用の tool は createAgent 側が responseFormat から自動で
    // 追加するため、ここでは通常 tool を空にするだけでよい。
    return handler({ ...request, tools: [] })
  },
})
