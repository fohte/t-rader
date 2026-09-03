import { createMiddleware } from 'langchain'
import { z } from 'zod'

// 通常 tool の呼び出しに上限がないと、構造化出力 (提出用 tool) を一度も
// 呼ばないままループが終わりうる (recursionLimit 到達で GraphRecursionError
// になるか、あるいは提出以外の tool を呼び続けて invoke が structuredResponse
// なしで解決する)。上限に達したターンでは提出用 tool 以外を外し、モデルに
// 選択の余地を残さないことで確実に提出させる。
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
    // 構造化出力用の tool は createAgent 側が responseFormat から自動で
    // 追加するため、ここでは通常 tool を空にするだけでよい。
    return handler({ ...request, tools: [] })
  },
})
