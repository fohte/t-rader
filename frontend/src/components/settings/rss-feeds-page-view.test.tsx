import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { RssFeedsPageView } from '#components/settings/rss-feeds-page-view'
import type { components } from '#lib/api/schema.gen'

vi.mock('#components/settings/rss-feed-dialog', () => ({
  RssFeedDialog: () => null,
}))
vi.mock('#components/settings/settings-back-link', () => ({
  SettingsBackLink: () => null,
}))

type RssFeed = components['schemas']['RssFeed']

const feed: RssFeed = {
  id: '00000000-0000-0000-0000-000000000001',
  source: 'sample-feed',
  display_name: 'Example Feed',
  url: 'https://feeds.example.invalid/sample.xml',
  enabled: true,
  content_source: 'none',
  created_at: '2099-01-01T00:00:00Z',
  updated_at: '2099-01-01T00:00:00Z',
}

function renderPage() {
  const actions = {
    add: 0,
    toggled: [] as RssFeed[],
    edited: [] as RssFeed[],
    deleted: [] as RssFeed[],
    dialogOpenChanges: [] as boolean[],
  }
  render(
    <RssFeedsPageView
      feeds={[feed]}
      isPending={false}
      dialogOpen={false}
      editingFeed={null}
      onAddFeed={() => {
        actions.add += 1
      }}
      onToggleEnabled={(value) => {
        actions.toggled.push(value)
      }}
      onEditFeed={(value) => {
        actions.edited.push(value)
      }}
      onDeleteFeed={(value) => {
        actions.deleted.push(value)
      }}
      onDialogOpenChange={(open) => {
        actions.dialogOpenChanges.push(open)
      }}
    />,
  )
  return actions
}

afterEach(() => {
  cleanup()
})

describe('RssFeedsPageView', () => {
  it('routes feed toggles and row actions to the selected feed', () => {
    const actions = renderPage()

    fireEvent.click(screen.getByRole('button', { name: 'フィードを追加' }))
    fireEvent.click(screen.getByRole('checkbox'))
    fireEvent.click(screen.getByRole('button', { name: '編集' }))
    fireEvent.click(screen.getByRole('button', { name: '削除' }))

    expect(actions).toEqual({
      add: 1,
      toggled: [feed],
      edited: [feed],
      deleted: [feed],
      dialogOpenChanges: [],
    })
  })
})
