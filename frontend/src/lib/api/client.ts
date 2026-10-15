import type { Middleware } from 'openapi-fetch'
import createFetchClient from 'openapi-fetch'
import createClient from 'openapi-react-query'

import type { paths } from '#lib/api/schema.gen'
import { parseJson } from '#lib/json'

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

const errorStatusMiddleware: Middleware = {
  async onResponse({ response }) {
    if (response.status < 400) return

    const parsed = parseJson(await response.clone().text())
    if (parsed.isErr() || !isRecord(parsed.value)) return

    const headers = new Headers(response.headers)
    headers.delete('content-length')
    headers.delete('content-encoding')
    headers.set('content-type', 'application/json')

    return new Response(
      JSON.stringify({ ...parsed.value, status: response.status }),
      {
        status: response.status,
        statusText: response.statusText,
        headers,
      },
    )
  },
}

// openapi-fetch クライアント (React 外でも利用可能)
export const fetchClient = createFetchClient<paths>({
  baseUrl: '/',
})
fetchClient.use(errorStatusMiddleware)

// openapi-react-query クライアント (React コンポーネント内で利用)
export const $api = createClient(fetchClient)
