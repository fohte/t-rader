import { Button } from '@fohte/ui/button'

import { NoteKindFilterSelect } from '#components/strategy-home/note-kind-filter-select'
import { NoteTagFilterSelect } from '#components/strategy-home/note-tag-filter-select'
import type { components } from '#lib/api/schema.gen'
import type { NoteKindCatalogState } from '#lib/note-kind-label'

type NoteKind = components['schemas']['NoteKind']

interface NotesFiltersProps {
  noteKinds: readonly NoteKind[]
  catalogState: NoteKindCatalogState
  kind: string | undefined
  tags: string[]
  tag: string | undefined
  onKindChange: (value: string | undefined) => void
  onTagChange: (value: string | undefined) => void
  onClear: () => void
}

export function NotesFilters({
  noteKinds,
  catalogState,
  kind,
  tags,
  tag,
  onKindChange,
  onTagChange,
  onClear,
}: NotesFiltersProps) {
  return (
    <div className="grid min-w-0 grid-cols-1 gap-2 sm:grid-cols-2 sm:items-center">
      <NoteKindFilterSelect
        noteKinds={noteKinds}
        catalogState={catalogState}
        value={kind}
        onChange={onKindChange}
      />
      <NoteTagFilterSelect
        tags={tags}
        value={tag}
        onChange={onTagChange}
        className="h-auto min-h-11 w-full min-w-0"
      />
      <Button
        variant="plain"
        onClick={onClear}
        className="justify-self-end sm:col-span-2"
      >
        条件を解除
      </Button>
    </div>
  )
}
