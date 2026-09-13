import { http, HttpResponse } from 'msw'

/**
 * GET /api/refs/resolve の MSW handler。names に無い token は name: null を返す。
 */
export function mockResolveRef(names: Record<string, string>) {
  return http.get('/api/refs/resolve', ({ request }) => {
    const link = new URL(request.url).searchParams.get('link') ?? ''
    const i = link.indexOf(':')
    return HttpResponse.json([
      {
        kind: i < 0 ? link : link.slice(0, i),
        id: i < 0 ? link : link.slice(i + 1),
        name: names[link] ?? null,
      },
    ])
  })
}
