import { Chip } from '@fohte/ui/chip'
import { Link } from '@tanstack/react-router'

import type { components } from '#lib/api/schema.gen'
import {
  type NoteKindCatalogState,
  type NoteKindSourceState,
  resolveNoteKindLabel,
} from '#lib/note-kind-label'

type NoteKind = components['schemas']['NoteKind']

interface NoteKindLabelProps {
  kind: string | null
  noteKinds: readonly NoteKind[]
  catalogState: NoteKindCatalogState
  sourceState?: NoteKindSourceState
  linkToFilter?: boolean
}

function getNoteKindText(
  kind: string | null,
  noteKinds: readonly NoteKind[],
  catalogState: NoteKindCatalogState,
  sourceState: NoteKindSourceState,
): string {
  if (sourceState === 'loading') return '種別を読み込み中'
  if (sourceState === 'error') return '種別を取得できません'
  if (kind == null || kind === '') return '種別なし'
  if (catalogState === 'loading') return '種別を読み込み中'
  if (catalogState === 'error') return '種別を取得できません'
  return resolveNoteKindLabel(kind, noteKinds) ?? '未登録の種別'
}

export function NoteKindLabel({
  kind,
  noteKinds,
  catalogState,
  sourceState = 'available',
  linkToFilter = false,
}: NoteKindLabelProps) {
  const label = getNoteKindText(kind, noteKinds, catalogState, sourceState)
  const knownKind =
    sourceState === 'available' &&
    kind != null &&
    kind !== '' &&
    catalogState === 'loaded' &&
    resolveNoteKindLabel(kind, noteKinds) != null
  const chip = <Chip tone="strong">{label}</Chip>
  const labelContent = (
    <span className="inline-flex max-w-full [&>[data-slot=chip]]:max-w-full [&>[data-slot=chip]]:break-words [&>[data-slot=chip]]:whitespace-normal">
      {chip}
    </span>
  )

  if (linkToFilter && knownKind) {
    return (
      <Link
        to="/notes"
        search={{ kind, tag: undefined, tab: undefined }}
        aria-label={`${label} のノートを表示`}
        className="inline-flex max-w-full rounded-sm outline-offset-2 focus-visible:outline-2 focus-visible:outline-ring"
      >
        {labelContent}
      </Link>
    )
  }

  return labelContent
}
