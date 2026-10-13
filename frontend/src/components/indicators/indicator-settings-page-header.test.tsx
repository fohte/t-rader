import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { IndicatorSettingsPageHeader } from '#components/indicators/indicator-settings-page-header'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

function getGlobalHeaderOutput(
  heading: HTMLElement,
  backLink: HTMLElement,
  description: HTMLElement,
) {
  return {
    heading: heading.textContent,
    backHref: backLink.getAttribute('href'),
    description: description.textContent,
  }
}

function getStrategyHeaderOutput(scopeLabel: HTMLElement, link: HTMLElement) {
  return {
    scopeLabel: scopeLabel.textContent,
    linkHref: link.getAttribute('href'),
  }
}

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

    expect(getGlobalHeaderOutput(heading, backLink, description)).toEqual({
      heading: '設定 — 自作指標 (共通)',
      backHref: '/settings',
      description:
        '全体の設定に登録した自作指標は、すべての戦略から参照できます。',
    })
  })

  it('戦略 scope に共通の指標へのリンクを表示する', async () => {
    renderHeader('strategy')
    const link = await screen.findByRole('link', {
      name: '共通の指標 (全体の設定) →',
    })

    const scopeLabel = await screen.findByText('この戦略だけの指標')

    expect(getStrategyHeaderOutput(scopeLabel, link)).toEqual({
      scopeLabel: '>この戦略だけの指標',
      linkHref: '/settings/indicators',
    })
  })
})
