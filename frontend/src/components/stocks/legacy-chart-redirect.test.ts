import { isRedirect } from '@tanstack/react-router'
import { describe, expect, it } from 'vitest'

import { Route as LegacyChartRoute } from '#routes/charts/$instrumentId'

function getRedirectOptions() {
  const beforeLoad = LegacyChartRoute.options.beforeLoad
  if (!beforeLoad) return null

  try {
    Reflect.apply(beforeLoad, undefined, [
      { params: { instrumentId: 'demo-code' } },
    ])
  } catch (error) {
    if (isRedirect(error)) {
      return { to: error.options.to, params: error.options.params }
    }
  }
  return null
}

describe('legacy chart route', () => {
  it('銘柄 ID を保ったまま全画面チャートへ転送する', () => {
    expect(getRedirectOptions()).toEqual({
      to: '/stocks/$stockId/chart',
      params: { stockId: 'demo-code' },
    })
  })
})
