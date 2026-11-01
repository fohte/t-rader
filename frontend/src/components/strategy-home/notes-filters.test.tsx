import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { NotesFilters } from '#components/strategy-home/notes-filters'
import type { components } from '#lib/api/schema.gen'

type NoteKind = components['schemas']['NoteKind']

const noteKinds: NoteKind[] = [
  {
    key: 'sample-kind',
    display_name: '架空の種別表示名',
    requires_approval: true,
    sort_order: 0,
  },
]

function getFilterChangeOutput(view: unknown, kind: unknown, tag: unknown) {
  return { view, kind, tag }
}

afterEach(cleanup)

describe('NotesFilters', () => {
  it('emits view, kind, and tag changes without losing the other selections', async () => {
    const user = userEvent.setup()
    const onViewChange = vi.fn()
    const onKindChange = vi.fn()
    const onTagChange = vi.fn()
    render(
      <NotesFilters
        noteKinds={noteKinds}
        catalogState="loaded"
        kind={undefined}
        tags={['sample-tag']}
        tag={undefined}
        view="all"
        onViewChange={onViewChange}
        onKindChange={onKindChange}
        onTagChange={onTagChange}
        onClear={() => {}}
      />,
    )

    await user.click(screen.getByRole('button', { name: '未読・レビュー待ち' }))
    await user.click(screen.getByRole('combobox', { name: '種別で絞り込み' }))
    await user.click(
      await screen.findByRole('option', { name: '架空の種別表示名' }),
    )
    await user.click(screen.getByRole('combobox', { name: 'タグで絞り込み' }))
    await user.click(await screen.findByRole('option', { name: 'sample-tag' }))

    expect(
      getFilterChangeOutput(
        onViewChange.mock.calls,
        onKindChange.mock.calls,
        onTagChange.mock.calls,
      ),
    ).toEqual({
      view: [['pending']],
      kind: [['sample-kind']],
      tag: [['sample-tag']],
    })
  })
})
