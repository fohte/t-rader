import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { IndicatorSettingsPageHeader } from '#components/indicators/indicator-settings-page-header'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

function renderHeader(scope: 'global' | 'strategy') {
  render(
    <RouterProvider
      router={createStoryRouter(
        () => (
          <IndicatorSettingsPageHeader scope={scope} />
        ),
        {
          paths: [
            '/settings',
            '/settings/indicators',
            '/strategies/$id/settings',
          ],
          initialPath:
            scope === 'global'
              ? '/settings/indicators'
              : '/strategies/demo-strategy/settings?settingsTab=indicators',
        },
      )}
    />,
  )
}

describe('IndicatorSettingsPageHeader', () => {
  it('共通 scope に戻り先と見出しを表示する', async () => {
    renderHeader('global')
    const heading = await screen.findByRole('heading', {
      name: '設定 — 自作指標 (共通)',
    })
    const backLink = await screen.findByRole('link', {
      name: '< 設定に戻る',
    })
    const description = await screen.findByText(
      '全体の設定に登録した自作指標は、すべての戦略から参照できます。',
    )

    expect(heading.textContent).toBe('設定 — 自作指標 (共通)')
    expect(backLink.getAttribute('href')).toBe('/settings')
    expect(description.textContent).toBe(
      '全体の設定に登録した自作指標は、すべての戦略から参照できます。',
    )
  })

  it('戦略 scope に共通の指標へのリンクを表示する', async () => {
    renderHeader('strategy')
    const link = await screen.findByRole('link', {
      name: '共通の指標 (全体の設定) →',
    })

    const scopeLabel = await screen.findByText('この戦略だけの指標')

    expect(scopeLabel.textContent).toBe('>この戦略だけの指標')
    expect(link.getAttribute('href')).toBe('/settings/indicators')
  })
})
