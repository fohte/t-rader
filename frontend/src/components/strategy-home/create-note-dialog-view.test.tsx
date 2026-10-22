import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import {
  CreateNoteDialogView,
  type CreateNoteDialogViewProps,
} from '#components/strategy-home/create-note-dialog-view'
import type { components } from '#lib/api/schema.gen'

type NoteKind = components['schemas']['NoteKind']

const noteKinds: NoteKind[] = [
  {
    key: 'sample-kind',
    display_name: 'サンプル種別',
    requires_approval: false,
    description: null,
    sort_order: 0,
  },
]

const baseProps: CreateNoteDialogViewProps = {
  open: true,
  onOpenChange: () => {},
  title: '',
  body: '',
  kind: '',
  noteKinds,
  noteKindsPending: false,
  noteKindsError: false,
  formError: null,
  isSubmitting: false,
  onTitleChange: () => {},
  onBodyChange: () => {},
  onKindChange: () => {},
  onSubmit: (event) => {
    event.preventDefault()
  },
}

function readClosedKindSelectOutput() {
  return {
    label: screen.getByRole('combobox', { name: '種別' }).textContent,
  }
}

afterEach(cleanup)

describe('CreateNoteDialogView', () => {
  it.each([
    {
      state: 'kind is empty',
      props: { kind: '' },
      expected: { label: '種別なし' },
    },
    {
      state: 'kinds are loading',
      props: { kind: '', noteKindsPending: true },
      expected: { label: '読み込み中…' },
    },
    {
      state: 'a kind is selected',
      props: { kind: 'sample-kind' },
      expected: { label: 'サンプル種別' },
    },
  ] as const)(
    'shows the selected label while the selector is closed: $state',
    ({ props, expected }) => {
      render(<CreateNoteDialogView {...baseProps} {...props} />)

      expect(readClosedKindSelectOutput()).toEqual(expected)
    },
  )
})
