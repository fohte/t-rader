import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { NoteVersionHeader } from '#components/note-detail/note-version-header'
import type { NoteVersion } from '#lib/api/note-version-types'
import { createStoryRouter } from '#storybook/story-router'

const version: NoteVersion = {
  id: 'fake-version-a',
  note_id: 'fake-note-a',
  version_no: 3,
  title: '架空データの確認メモ',
  body_md: '変更された本文です。',
  frontmatter_json: {},
  resolved_price_references_json: {},
  graphs_json: [],
  status: 'unread',
  is_current: false,
  change_reason: null,
  created_by_kind: 'llm',
  execution_id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
  created_at: '2026-01-12T02:00:00Z',
  reviewed_at: null,
}

afterEach(cleanup)

async function readGenerationLinks() {
  const strategyLink = await screen.findByRole('link', { name: '架空戦略' })
  const runLink = await screen.findByRole('link', { name: '実行詳細 →' })

  return [
    {
      text: strategyLink.textContent,
      href: strategyLink.getAttribute('href'),
    },
    { text: runLink.textContent, href: runLink.getAttribute('href') },
  ]
}

describe('NoteVersionHeader', () => {
  it('links the strategy and run that generated the displayed version', async () => {
    const router = createStoryRouter(
      () => (
        <NoteVersionHeader
          version={version}
          noteKind={null}
          hasNoteError={false}
          executionOrigin={{
            strategyId: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
            strategyName: '架空戦略',
            taskId: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
          }}
          onAsk={() => {}}
        />
      ),
      {
        paths: ['/strategies/$id', '/strategies/$id/runs/$taskId'],
        initialPath: '/strategies/sample-strategy',
      },
    )
    render(<RouterProvider router={router} />)

    expect(await readGenerationLinks()).toEqual([
      {
        text: '架空戦略',
        href: '/strategies/bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
      },
      {
        text: '実行詳細 →',
        href: '/strategies/bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb/runs/cccccccc-cccc-4ccc-8ccc-cccccccccccc',
      },
    ])
  })

  it('does not show origin links for a version written by a user', async () => {
    const router = createStoryRouter(
      () => (
        <NoteVersionHeader
          version={{ ...version, created_by_kind: 'human', execution_id: null }}
          noteKind={null}
          hasNoteError={false}
          executionOrigin={{
            strategyId: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
            strategyName: '架空戦略',
            taskId: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
          }}
          onAsk={() => {}}
        />
      ),
      {
        paths: ['/strategies/$id', '/strategies/$id/runs/$taskId'],
        initialPath: '/strategies/sample-strategy',
      },
    )
    render(<RouterProvider router={router} />)

    await screen.findByRole('heading', { name: version.title })
    expect(screen.queryByRole('link', { name: '実行詳細 →' })).toBeNull()
  })
})
