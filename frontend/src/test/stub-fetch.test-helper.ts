import { vi } from 'vitest'

export type FetchHandler = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>

let fetchHandler: FetchHandler | null = null

function activeFetch(
  input: RequestInfo | URL,
  init?: RequestInit,
): Promise<Response> {
  return (
    fetchHandler?.(input, init) ??
    Promise.resolve(new Response(null, { status: 500 }))
  )
}

export function stubFetch(handler: FetchHandler): void {
  fetchHandler = handler
  vi.stubGlobal('fetch', activeFetch)
}

export function resetFetch(): void {
  fetchHandler = null
  vi.unstubAllGlobals()
}
