import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { NoteKindFilterSelect } from '#components/strategy-home/note-kind-filter-select'
import type { components } from '#lib/api/schema.gen'

type NoteKind = components['schemas']['NoteKind']

const noteKinds: NoteKind[] = [
  {
    key: 'sample-kind',
    display_name: '架空の種別表示名',
    requires_approval: false,
    sort_order: 0,
  },
]

afterEach(cleanup)

function getKnownKindSelectOutput(select: HTMLElement) {
  return {
    selected: select.textContent,
    options: screen.getAllByRole('option').map((option) => option.textContent),
  }
}

function getClearedUnknownKindOutput(
  select: HTMLElement,
  onChangeCalls: unknown[][],
) {
  return {
    selected: select.textContent,
    keyVisible: screen.queryByText('unlisted-kind-key') != null,
    onChange: onChangeCalls,
  }
}

describe('NoteKindFilterSelect', () => {
  it('shows the selected display name and catalog options', async () => {
    const user = userEvent.setup()
    render(
      <NoteKindFilterSelect
        noteKinds={noteKinds}
        catalogState="loaded"
        value="sample-kind"
        onChange={() => {}}
      />,
    )

    const select = screen.getByRole('combobox', { name: '種別で絞り込み' })
    await user.click(select)
    await screen.findAllByRole('option')

    expect(getKnownKindSelectOutput(select)).toEqual({
      selected: '架空の種別表示名',
      options: ['すべての種別', '架空の種別表示名'],
    })
  })

  it('keeps an unknown URL kind visible without exposing its key and can clear it', async () => {
    const onChange = vi.fn()
    const user = userEvent.setup()
    render(
      <NoteKindFilterSelect
        noteKinds={noteKinds}
        catalogState="loaded"
        value="unlisted-kind-key"
        onChange={onChange}
      />,
    )

    const select = screen.getByRole('combobox', { name: '種別で絞り込み' })
    await user.click(select)
    await user.click(
      await screen.findByRole('option', { name: 'すべての種別' }),
    )

    expect(getClearedUnknownKindOutput(select, onChange.mock.calls)).toEqual({
      selected: '未登録の種別',
      keyVisible: false,
      onChange: [[undefined]],
    })
  })

  it.each([
    { state: 'loading' as const, label: '種別を読み込み中' },
    { state: 'error' as const, label: '種別を取得できません' },
  ])(
    'retains a selected key while catalog state is $state',
    ({ state, label }) => {
      render(
        <NoteKindFilterSelect
          noteKinds={[]}
          catalogState={state}
          value="unlisted-kind-key"
          onChange={() => {}}
        />,
      )

      expect(
        screen.getByRole('combobox', { name: '種別で絞り込み' }).textContent,
      ).toEqual(label)
    },
  )
})
