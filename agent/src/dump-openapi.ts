import type { A2ARequestHandler } from '@a2a-js/sdk/server'
import { OpenAPIHono } from '@hono/zod-openapi'

import { mountInternalApiRoutes } from '#internal-api/routes'

const notImplemented = (): never => {
  // eslint-disable-next-line no-restricted-syntax -- ドキュメント生成は route 定義のスキーマだけを読み handler を呼ばない契約。呼ばれたらその契約が破れたバグなので落とす
  throw new Error('not implemented: openapi generation never invokes handlers')
}

// ドキュメント生成は route 定義の静的なスキーマだけを読むので、handler は
// 呼ばれない。全メソッドを notImplemented にした最小のダミーで足りる。
const dummyRequestHandler: A2ARequestHandler = {
  getAgentCard: notImplemented,
  getAuthenticatedExtendedAgentCard: notImplemented,
  sendMessage: notImplemented,
  sendMessageStream: notImplemented,
  getTask: notImplemented,
  cancelTask: notImplemented,
  setTaskPushNotificationConfig: notImplemented,
  getTaskPushNotificationConfig: notImplemented,
  listTaskPushNotificationConfigs: notImplemented,
  deleteTaskPushNotificationConfig: notImplemented,
  resubscribe: notImplemented,
}

const app = new OpenAPIHono()
mountInternalApiRoutes(app, { requestHandler: dummyRequestHandler })

const document = app.getOpenAPI31Document({
  openapi: '3.1.0',
  info: {
    title: 't-rader-agent internal API',
    version: '0.0.0',
  },
})

console.log(JSON.stringify(document, null, 2))
