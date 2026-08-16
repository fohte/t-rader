import type { A2ARequestHandler } from '@a2a-js/sdk/server'
import { OpenAPIHono } from '@hono/zod-openapi'

import { mountInternalApiRoutes } from '#internal-api/routes'

const unreachable = (): never => {
  // eslint-disable-next-line no-restricted-syntax -- ドキュメント生成は route 定義のスキーマだけを読み handler を呼ばない契約。呼ばれたらその契約が破れたバグなので落とす
  throw new Error('unreachable: openapi generation never invokes handlers')
}

// ドキュメント生成は route 定義の静的なスキーマだけを読むので、handler は
// 呼ばれない。全メソッドを unreachable にした最小のダミーで足りる。
const dummyRequestHandler: A2ARequestHandler = {
  getAgentCard: unreachable,
  getAuthenticatedExtendedAgentCard: unreachable,
  sendMessage: unreachable,
  sendMessageStream: unreachable,
  getTask: unreachable,
  cancelTask: unreachable,
  setTaskPushNotificationConfig: unreachable,
  getTaskPushNotificationConfig: unreachable,
  listTaskPushNotificationConfigs: unreachable,
  deleteTaskPushNotificationConfig: unreachable,
  resubscribe: unreachable,
}

const app = new OpenAPIHono()
mountInternalApiRoutes(app, { requestHandler: dummyRequestHandler })

// backend 側は progenitor で client を生成しており、progenitor は
// OpenAPI 3.0.x までしか読めない (openapiv3 crate が 3.0.x 専用のため) ので
// 3.1 ではなく 3.0 で出力する。
const document = app.getOpenAPIDocument({
  openapi: '3.0.0',
  info: {
    title: 't-rader-agent internal API',
    version: '0.0.0',
  },
})

console.log(JSON.stringify(document, null, 2))
