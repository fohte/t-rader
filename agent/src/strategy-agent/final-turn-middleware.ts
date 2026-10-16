import { captureWithFingerprint } from '@fohte/service-kit/observability'
import { createMiddleware } from 'langchain'
import { z } from 'zod'

import { logger } from '#logger'

const FINAL_TURN_FORCED_FINGERPRINT = 'final-turn-middleware.forced-submission'

// 通常 tool の呼び出し上限。到達ターンでは提出用 tool 以外を外して構造化出力を強制する。
export const MAX_MODEL_CALLS_PER_INVOKE = 15

export interface ModelCallBudget {
  readonly maxModelCalls: number
  count: number
}

export const createFinalTurnMiddleware = (
  maxModelCalls: number = MAX_MODEL_CALLS_PER_INVOKE,
  budget?: ModelCallBudget,
) =>
  createMiddleware({
    name: 'finalTurnMiddleware',
    stateSchema: z.object({
      modelCallCount: z.number().default(0),
    }),
    beforeModel: (state) => {
      const modelCallCount =
        budget === undefined ? state.modelCallCount + 1 : ++budget.count
      return { modelCallCount }
    },
    wrapModelCall: (request, handler) => {
      const modelCallCount =
        budget === undefined ? request.state.modelCallCount : budget.count
      if (budget !== undefined && modelCallCount > maxModelCalls) {
        return Promise.reject(
          new Error(
            `finalTurnMiddleware: model-call limit ${String(maxModelCalls)} exceeded`,
          ),
        )
      }
      if (modelCallCount < maxModelCalls) return handler(request)
      const error = new Error(
        `finalTurnMiddleware: forcing structured-output submission at model call ${String(modelCallCount)}`,
      )
      logger.warn({}, error.message)
      captureWithFingerprint(error, FINAL_TURN_FORCED_FINGERPRINT, {
        level: 'warning',
      })
      // 構造化出力用の tool は createAgent 側が responseFormat から自動で
      // 追加するため、ここでは通常 tool を空にするだけでよい。
      return handler({ ...request, tools: [] })
    },
  })

export const finalTurnMiddleware = createFinalTurnMiddleware()
