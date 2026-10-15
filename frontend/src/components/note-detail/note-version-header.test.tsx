import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { NoteVersionHeader } from '#components/note-detail/note-version-header'
import type { NoteVersion } from '#lib/api/note-version-types'

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
  execution_id: null,
  created_at: '2026-01-12T02:00:00Z',
  reviewed_at: null,
}

afterEach(cleanup)

function getHeaderOutput() {
  return {
    kindLabel: screen.getByText('架空の種別表示名').textContent,
    title: screen.getByRole('heading', { level: 1 }).textContent,
  }
}

describe('NoteVersionHeader', () => {
  it('shows the resolved kind label for a version awaiting review', () => {
    render(
      <NoteVersionHeader
        version={version}
        noteKindLabel="架空の種別表示名"
        hasNoteError={false}
        onAsk={() => {}}
      />,
    )

    expect(getHeaderOutput()).toEqual({
      kindLabel: '架空の種別表示名',
      title: '架空の確認メモ',
    })
  })
})
