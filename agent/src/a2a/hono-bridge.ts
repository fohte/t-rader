import type { AgentCard, JSONRPCResponse } from '@a2a-js/sdk'
import { AGENT_CARD_PATH, Extensions, HTTP_EXTENSION_HEADER } from '@a2a-js/sdk'
import {
  type A2ARequestHandler,
  JsonRpcTransportHandler,
  ServerCallContext,
} from '@a2a-js/sdk/server'
import type { Hono } from 'hono'
import { streamSSE } from 'hono/streaming'

export interface A2aHonoBridgeOptions {
  agentCard: AgentCard
  requestHandler: A2ARequestHandler
  // Cluster-internal auth: when set, JSON-RPC POST requests must present
  // `Authorization: Bearer <bearerToken>`. The agent card GET route stays
  // unauthenticated so remote agents can discover capabilities first.
  bearerToken?: string
}

const isAsyncGenerator = (
  value: JSONRPCResponse | AsyncGenerator<JSONRPCResponse, void, undefined>,
): value is AsyncGenerator<JSONRPCResponse, void, undefined> =>
  // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition -- defends against a runtime value that doesn't match its declared type (e.g. an SDK returning something unexpected)
  typeof value === 'object' && value !== null && Symbol.asyncIterator in value

export const mountA2aRoutes = (
  app: Hono,
  options: A2aHonoBridgeOptions,
): void => {
  const { agentCard, requestHandler, bearerToken } = options
  const transportHandler = new JsonRpcTransportHandler(requestHandler)

  app.get(`/${AGENT_CARD_PATH}`, (c) => c.json(agentCard))

  app.post('/', async (c) => {
    if (bearerToken !== undefined) {
      const header = c.req.header('authorization')
      if (header !== `Bearer ${bearerToken}`) {
        return c.json(
          {
            jsonrpc: '2.0',
            id: null,
            error: { code: -32000, message: 'Unauthorized' },
          },
          401,
        )
      }
    }

    const body: unknown = await c.req.json()
    const context = new ServerCallContext(
      Extensions.parseServiceParameter(c.req.header(HTTP_EXTENSION_HEADER)),
    )
    const result = await transportHandler.handle(body, context)

    if (isAsyncGenerator(result)) {
      return streamSSE(c, async (stream) => {
        for await (const event of result) {
          await stream.writeSSE({ data: JSON.stringify(event) })
        }
      })
    }
    return c.json(result)
  })
}
