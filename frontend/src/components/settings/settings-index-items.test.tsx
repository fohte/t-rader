import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { SettingsIndexItems } from '#components/settings/settings-index-items'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

describe('SettingsIndexItems', () => {
  it('共通の自作指標を含む全設定へのリンクを表示する', async () => {
    render(
      <RouterProvider
        router={createStoryRouter(
          () => (
            <SettingsIndexItems />
          ),
          {
            paths: [
              '/settings',
              '/settings/agent-configs',
              '/settings/rss-feeds',
              '/settings/ingest-status',
              '/settings/risk-policy',
              '/settings/indicators',
            ],
            initialPath: '/settings',
          },
        )}
      />,
    )

    const list = await screen.findByRole('list')
    const links = within(list)
      .getAllByRole('link')
      .map((link) => {
        const content = link.querySelectorAll(':scope > div > div')
        return {
          label: content.item(0).textContent,
          description: content.item(1).textContent,
          href: link.getAttribute('href'),
        }
      })

    expect(links).toEqual([
      {
        label: 'Agent 設定',
        description: '目的ごとの AGENTS.md / skills / agent graph を管理する',
        href: '/settings/agent-configs',
      },
      {
        label: 'RSS フィード',
        description: 'ニュース集約対象の公開 RSS を追加・編集・無効化する',
        href: '/settings/rss-feeds',
      },
      {
        label: '取り込み状況',
        description: '取り込みジョブの実行履歴とデータ状況を確認する',
        href: '/settings/ingest-status',
      },
      {
        label: 'リスク上限',
        description:
          '口座全体の保有銘柄時価合計に対するセクター集中度の上限を設定する',
        href: '/settings/risk-policy',
      },
      {
        label: '自作指標 (共通)',
        description: 'すべての戦略で使う自作指標を管理する',
        href: '/settings/indicators',
      },
    ])
  })
})
