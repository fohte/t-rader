import { Chip } from '@fohte/ui/chip'

import { StatusPill } from '#components/strategy-home/status-pill'
import type { components } from '#lib/api/schema.gen'

type NoteOverview = components['schemas']['NoteOverviewResponse']

export function NoteOverviewStatus({ overview }: { overview: NoteOverview }) {
  const note = overview.representative

  return (
    <span className="flex flex-wrap items-center gap-2 font-mono text-2xs">
      <StatusPill status={note.status} />
      {!note.is_current && overview.current == null && <Chip>現行なし</Chip>}
      {overview.unread_version_count > 0 && (
        <Chip>未読 {String(overview.unread_version_count)} バージョン</Chip>
      )}
    </span>
  )
}
