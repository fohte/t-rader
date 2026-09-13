import type { Middleware } from 'openapi-fetch'

import { fetchClient } from '#lib/api/client'

export interface RefResolveStub {
  kind: string
  id: string
  name: string
}

// /api/refs/resolve の openapi-fetch middleware。stubs に一致する token のみ
// 解決済みとして返し、それ以外は未解決 (name: null) を返す
export function installRefResolveMock(stubs: RefResolveStub[] = []) {
  const middleware: Middleware = {
    onRequest({ request }) {
      const url = new URL(request.url)
      if (!url.pathname.endsWith('/api/refs/resolve')) return undefined
      const links = (url.searchParams.get('link') ?? '').split(',')
      const body = links.map((link) => {
        const stub = stubs.find((s) => `${s.kind}:${s.id}` === link)
        const [kind = '', ...rest] = link.split(':')
        return {
          kind: stub?.kind ?? kind,
          id: stub?.id ?? rest.join(':'),
          name: stub?.name ?? null,
        }
      })
      return new Response(JSON.stringify(body), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      })
    },
  }
  fetchClient.use(middleware)
  return () => {
    fetchClient.eject(middleware)
  }
}
