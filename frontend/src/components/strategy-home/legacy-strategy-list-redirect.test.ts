import { isRedirect } from '@tanstack/react-router'
import { describe, expect, it } from 'vitest'

import { validateStrategyListSearch } from '#components/strategy-home/strategy-list-tabs'
import { Route as RunsRoute } from '#routes/runs'
import { Route as StrategyIndicatorsRoute } from '#routes/strategies/$id/indicators'
import { Route as StrategySettingsRoute } from '#routes/strategies/$id/settings'
import { Route as StrategyListRoute } from '#routes/strategies/index'

type LegacyRoute =
  | typeof RunsRoute
  | typeof StrategyIndicatorsRoute
  | typeof StrategySettingsRoute
  | typeof StrategyListRoute

function getRedirectOptions(route: LegacyRoute, args: unknown[] = []) {
  const beforeLoad = route.options.beforeLoad
  if (!beforeLoad) return null

  try {
    Reflect.apply(beforeLoad, undefined, args)
  } catch (error) {
    if (isRedirect(error)) {
      return {
        to: error.options.to,
        ...(error.options.params != null
          ? { params: error.options.params }
          : {}),
        ...(error.options.search != null
          ? { search: error.options.search }
          : {}),
        ...(error.options.replace != null
          ? { replace: error.options.replace }
          : {}),
      }
    }
  }
  return null
}

describe('legacy strategy indicators routes', () => {
  it('旧 /runs を戦略一覧の実行履歴へ redirect する', () => {
    expect(getRedirectOptions(RunsRoute)).toEqual({
      to: '/strategies',
      search: { strategyTab: 'runs' },
    })
  })

  it('戦略用の自作指標 URL を設定タブへ redirect する', () => {
    expect(
      getRedirectOptions(StrategyIndicatorsRoute, [
        { params: { id: 'demo-strategy' } },
      ]),
    ).toEqual({
      to: '/strategies/$id/settings',
      params: { id: 'demo-strategy' },
      search: { settingsTab: 'indicators' },
    })
  })

  it('旧戦略一覧の indicators タブを全体設定へ redirect する', () => {
    expect(
      getRedirectOptions(StrategyListRoute, [
        { search: { legacyIndicatorsTab: true } },
      ]),
    ).toEqual({ to: '/settings/indicators' })
  })

  it('戦略一覧は indicators を旧値として検出する', () => {
    expect(validateStrategyListSearch({ strategyTab: 'indicators' })).toEqual({
      legacyIndicatorsTab: true,
    })
  })

  it('戦略設定の search param がない場合は全般タブの URL へ redirect する', () => {
    expect(
      getRedirectOptions(StrategySettingsRoute, [
        { params: { id: 'demo-strategy' }, search: {} },
      ]),
    ).toEqual({
      to: '/strategies/$id/settings',
      params: { id: 'demo-strategy' },
      search: { settingsTab: 'general' },
      replace: true,
    })
  })
})
