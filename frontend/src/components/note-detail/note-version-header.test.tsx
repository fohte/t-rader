import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { NoteVersionHeader } from '#components/note-detail/note-version-header'
import type { NoteVersion } from '#lib/api/note-version-types'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'
import { getExecutionOriginLinks } from '#test/execution-origin-links.test-helper'

type NoteKind = components['schemas']['NoteKind']

const version: NoteVersion = {
  id: 'fake-version-a',
  note_id: 'fake-note-a',
  version_no: 1,
  title: '架空の確認メモ',
  body_md: '架空の本文',
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

const noteKinds: NoteKind[] = [
  {
    key: 'sample-kind',
    display_name: '架空の種別表示名',
    requires_approval: false,
    sort_order: 0,
  },
]

afterEach(cleanup)

function getHeaderOutput() {
  return {
    kindLabel:
      screen.queryByText('架空の種別表示名')?.textContent ??
      screen.queryByText('種別なし')?.textContent ??
      screen.queryByText('種別を読み込み中')?.textContent ??
      screen.queryByText('種別を取得できません')?.textContent ??
      screen.queryByText('ノートの種別を読み込み中')?.textContent ??
      screen.queryByText('ノートの種別を取得できません')?.textContent ??
      screen.queryByText('未登録の種別')?.textContent ??
      null,
    title: screen.getByRole('heading', { level: 1 }).textContent,
    originLinks: getExecutionOriginLinks().map((link) => ({
      name: link.textContent,
      href: link.getAttribute('href'),
    })),
    kindLink:
      screen
        .queryByRole('link', { name: '架空の種別表示名 のノートを表示' })
        ?.getAttribute('href') ?? null,
  }
}

function getHeaderOutputWithLabelOrder() {
  return {
    ...getHeaderOutput(),
    labelPrecedesTitle:
      screen
        .getByText('架空の種別表示名')
        .compareDocumentPosition(screen.getByRole('heading', { level: 1 })) &
      Node.DOCUMENT_POSITION_FOLLOWING,
  }
}

function getUnregisteredHeaderOutput() {
  return {
    ...getHeaderOutput(),
    rawKeyVisible: screen.queryByText('unlisted-kind-key') != null,
  }
}

describe('NoteVersionHeader', () => {
  it('shows the strategy and run that generated the displayed version', async () => {
    const router = createStoryRouter(
      () => (
        <NoteVersionHeader
          version={version}
          noteKind={null}
          noteKinds={noteKinds}
          noteKindCatalogState="loaded"
          noteKindSourceState="available"
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

    const strategyLink = await screen.findByRole('link', { name: '架空戦略' })
    expect(strategyLink.parentElement?.textContent).toEqual(
      '架空戦略 · 実行詳細 →',
    )
  })

  it('does not show origin links for a version written by a user', async () => {
    const router = createStoryRouter(
      () => (
        <NoteVersionHeader
          version={{ ...version, created_by_kind: 'human', execution_id: null }}
          noteKind={null}
          noteKinds={noteKinds}
          noteKindCatalogState="loaded"
          noteKindSourceState="available"
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
    expect(getExecutionOriginLinks()).toEqual([])
  })

  it('places the registered kind before the title and links to its filtered list', async () => {
    const router = createStoryRouter(
      () => (
        <NoteVersionHeader
          version={version}
          noteKind="sample-kind"
          noteKinds={noteKinds}
          noteKindCatalogState="loaded"
          noteKindSourceState="available"
          executionOrigin={null}
          onAsk={() => {}}
        />
      ),
      { paths: ['/notes'], initialPath: '/notes' },
    )
    render(<RouterProvider router={router} />)
    await screen.findByRole('heading', { name: version.title })

    expect(getHeaderOutputWithLabelOrder()).toEqual({
      kindLabel: '架空の種別表示名',
      title: version.title,
      originLinks: [],
      kindLink: '/notes?kind=sample-kind',
      labelPrecedesTitle: Node.DOCUMENT_POSITION_FOLLOWING,
    })
  })

  it('does not link an unregistered kind or expose its key', async () => {
    const router = createStoryRouter(
      () => (
        <NoteVersionHeader
          version={version}
          noteKind="unlisted-kind-key"
          noteKinds={[]}
          noteKindCatalogState="loaded"
          noteKindSourceState="available"
          executionOrigin={null}
          onAsk={() => {}}
        />
      ),
      { paths: ['/notes'], initialPath: '/notes' },
    )
    render(<RouterProvider router={router} />)
    await screen.findByRole('heading', { name: version.title })

    expect(getUnregisteredHeaderOutput()).toEqual({
      kindLabel: '未登録の種別',
      title: version.title,
      originLinks: [],
      kindLink: null,
      rawKeyVisible: false,
    })
  })

  it.each([
    ['loading', 'ノートの種別を読み込み中'],
    ['error', 'ノートの種別を取得できません'],
  ] as const)(
    'distinguishes when the note kind is %s',
    async (noteKindSourceState, kindLabel) => {
      const router = createStoryRouter(
        () => (
          <NoteVersionHeader
            version={version}
            noteKind="sample-kind"
            noteKinds={noteKinds}
            noteKindCatalogState="loaded"
            noteKindSourceState={noteKindSourceState}
            executionOrigin={null}
            onAsk={() => {}}
          />
        ),
        { paths: ['/notes'], initialPath: '/notes' },
      )
      render(<RouterProvider router={router} />)
      await screen.findByRole('heading', { name: version.title })

      expect(getHeaderOutput()).toEqual({
        kindLabel,
        title: version.title,
        originLinks: [],
        kindLink: null,
      })
    },
  )
})
