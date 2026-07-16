import { Hono } from 'hono'
import { describe, expect, it } from 'vitest'

import { bearerAuth } from '@/internal-api/auth'

const buildApp = (token: string): Hono => {
  const app = new Hono()
  app.use('/*', bearerAuth(token))
  app.get('/ping', (c) => c.json({ ok: true }))
  return app
}

describe('bearerAuth', () => {
  it('allows a request with the matching bearer token', async () => {
    const app = buildApp('secret')
    const res = await app.request('/ping', {
      headers: { authorization: 'Bearer secret' },
    })
    expect(res.status).toBe(200)
    expect(await res.json()).toEqual({ ok: true })
  })

  it('rejects a request with no authorization header', async () => {
    const app = buildApp('secret')
    const res = await app.request('/ping')
    expect(res.status).toBe(401)
    expect(await res.json()).toEqual({ error: 'unauthorized' })
  })

  it('rejects a request with a mismatched token', async () => {
    const app = buildApp('secret')
    const res = await app.request('/ping', {
      headers: { authorization: 'Bearer wrong' },
    })
    expect(res.status).toBe(401)
  })

  it('rejects a request missing the Bearer prefix', async () => {
    const app = buildApp('secret')
    const res = await app.request('/ping', {
      headers: { authorization: 'secret' },
    })
    expect(res.status).toBe(401)
  })
})
