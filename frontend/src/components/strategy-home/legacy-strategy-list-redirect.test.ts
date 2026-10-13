import { isRedirect } from '@tanstack/react-router'
import { describe, expect, it } from 'vitest'

import { Route as RunsRoute } from '#routes/runs'

type LegacyRoute = typeof RunsRoute

function getRedirectOptions(route: LegacyRoute) {
  const beforeLoad = route.options.beforeLoad
  if (!beforeLoad) return null

  try {
    Reflect.apply(beforeLoad, undefined, [])
  } catch (error) {
    if (isRedirect(error)) {
      return { to: error.options.to, search: error.options.search }
    }
  }
  return null
}

describe('legacy strategy list routes', () => {
  it.each([
    {
      legacyPath: '/runs',
      route: RunsRoute,
      expected: { to: '/strategies', search: { strategyTab: 'runs' } },
    },
  ] as const)(
    '$legacyPath を対応する戦略タブへ redirect する',
    ({ route, expected }) => {
      expect(getRedirectOptions(route)).toEqual(expected)
    },
  )
})
