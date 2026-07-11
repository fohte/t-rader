import { timingSafeEqual } from 'node:crypto'

import type { MiddlewareHandler } from 'hono'

const BEARER_PREFIX = 'Bearer '

const constantTimeEquals = (a: string, b: string): boolean => {
  const bufA = Buffer.from(a)
  const bufB = Buffer.from(b)
  // timingSafeEqual throws on mismatched lengths, so an early inequality
  // return here does not leak timing information about the token itself
  // (only about the presented value's length, which is not secret).
  if (bufA.length !== bufB.length) return false
  return timingSafeEqual(bufA, bufB)
}

export const bearerAuth = (token: string): MiddlewareHandler => {
  return async (c, next) => {
    const header = c.req.header('authorization')
    const presented = header?.startsWith(BEARER_PREFIX)
      ? header.slice(BEARER_PREFIX.length)
      : undefined
    if (presented === undefined || !constantTimeEquals(presented, token)) {
      return c.json({ error: 'unauthorized' }, 401)
    }
    return next()
  }
}
