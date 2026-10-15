import type { components } from '#lib/api/schema.gen'

type NoteKindLabelSource = Pick<
  components['schemas']['NoteKind'],
  'key' | 'display_name'
>

export function resolveNoteKindLabel(
  kind: string | null,
  noteKinds: readonly NoteKindLabelSource[],
): string | null {
  if (kind == null) return null
  return (
    noteKinds.find((noteKind) => noteKind.key === kind)?.display_name ?? kind
  )
}
