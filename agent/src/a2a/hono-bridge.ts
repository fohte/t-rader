import type { AgentCard, JSONRPCResponse } from '@a2a-js/sdk'
import { AGENT_CARD_PATH, Extensions, HTTP_EXTENSION_HEADER } from '@a2a-js/sdk'
import {
  type A2ARequestHandler,
  JsonRpcTransportHandler,
  ServerCallContext,
} from '@a2a-js/sdk/server'
import { captureWithFingerprint } from '@fohte/service-kit/observability'
import type { Hono } from 'hono'
import { streamSSE } from 'hono/streaming'

const STREAM_FAILED_FINGERPRINT = 'a2a.hono.stream-failed'

export interface A2aHonoBridgeOptions {
  agentCard: AgentCard
  requestHandler: A2ARequestHandler
  // Cluster-internal auth: when set, JSON-RPC POST requests must present
  // `Authorization: Bearer <bearerToken>`. The agent card GET route stays
  // unauthenticated so remote agents can discover capabilities first.
  bearerToken?: string
}

const internalErrorResponse = (
  err: unknown,
  requestId: string | number | null,
): JSONRPCResponse => ({
  jsonrpc: '2.0',
  id: requestId,
  error: {
    code: -32603,
    message: err instanceof Error ? err.message : String(err),
  },
})

// The SDK client matches an SSE event's `id` against the request it sent
// (chunk-S53FFHPM.js's JsonRpcTransportHandler.handle echoes `rpcRequest.id`
// the same way) before even looking at `error`, so an error event stamped
// with the wrong id surfaces as an "ID mismatch" instead of this message.
const requestIdOf = (body: unknown): string | number | null => {
  if (typeof body !== 'object' || body === null) return null
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- body is an untyped JSON-RPC request; the `id` field is narrowed immediately below via typeof
  const id = (body as Record<string, unknown>)['id']
  return typeof id === 'string' || typeof id === 'number' ? id : null
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
    const requestId = requestIdOf(body)
    const context = new ServerCallContext(
      Extensions.parseServiceParameter(c.req.header(HTTP_EXTENSION_HEADER)),
    )
    const result = await transportHandler.handle(body, context)

    if (isAsyncGenerator(result)) {
      return streamSSE(
        c,
        async (stream) => {
          for await (const event of result) {
            await stream.writeSSE({ data: JSON.stringify(event) })
          }
        },
        async (err, stream) => {
          console.error('a2a JSON-RPC stream failed:', err)
          captureWithFingerprint(err, STREAM_FAILED_FINGERPRINT)
          await stream
            .writeSSE({
              event: 'error',
              data: JSON.stringify(internalErrorResponse(err, requestId)),
            })
            .catch(() => {
              // The connection is likely already closed; nothing more to do.
            })
        },
      )
    }
    return c.json(result)
  })
}
