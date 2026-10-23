import type { components } from '#lib/api/schema.gen'

type NoteKindLabelSource = Pick<
  components['schemas']['NoteKind'],
  'key' | 'display_name'
>

export function resolveNoteKindLabel(
  kind: string | null,
  noteKinds: readonly NoteKindLabelSource[],
): string | null {
  if (kind == null || kind === '') return null
  const displayName = noteKinds.find(
    (noteKind) => noteKind.key === kind,
  )?.display_name
  return displayName == null || displayName.trim() === '' ? null : displayName
}

export type NoteKindCatalogState = 'loading' | 'error' | 'loaded'
export type NoteKindSourceState = 'loading' | 'error' | 'available'
