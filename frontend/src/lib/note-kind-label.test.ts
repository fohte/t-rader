import { describe, expect, it } from 'vitest'

import { resolveNoteKindLabel } from '#lib/note-kind-label'

describe('resolveNoteKindLabel', () => {
  const noteKinds = [
    { key: 'fabricated-kind-key', display_name: '架空の種別表示名' },
  ]

  it('returns the display name for a known kind', () => {
    expect(resolveNoteKindLabel('fabricated-kind-key', noteKinds)).toBe(
      '架空の種別表示名',
    )
  })

  it('returns null when the kind is unknown', () => {
    expect(resolveNoteKindLabel('unlisted-kind-key', noteKinds)).toBe(null)
  })

  it('returns null when the note has no kind', () => {
    expect(resolveNoteKindLabel(null, noteKinds)).toBeNull()
  })

  it('returns null when the kind key is empty', () => {
    expect(resolveNoteKindLabel('', noteKinds)).toBeNull()
  })
})
