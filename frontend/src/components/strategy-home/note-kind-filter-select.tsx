import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@fohte/ui/select'

import {
  decodePrefixedSelectValue,
  encodePrefixedSelectValue,
} from '#components/strategy-home/prefixed-select-value'
import type { components } from '#lib/api/schema.gen'
import type { NoteKindCatalogState } from '#lib/note-kind-label'

type NoteKind = components['schemas']['NoteKind']

const ALL_KINDS_VALUE = '__all-kinds__'
const KIND_VALUE_PREFIX = 'kind:'
const ALL_KINDS_LABEL = 'すべての種別'

interface NoteKindFilterSelectProps {
  noteKinds: readonly NoteKind[]
  catalogState: NoteKindCatalogState
  value: string | undefined
  defaultOpen?: boolean
  onChange: (value: string | undefined) => void
}

function getSelectedValueLabel(
  value: string | undefined,
  noteKinds: readonly NoteKind[],
  catalogState: NoteKindCatalogState,
): string {
  if (value == null || value === '') return ALL_KINDS_LABEL
  if (catalogState === 'loading') return '種別を読み込み中'
  if (catalogState === 'error') return '種別を取得できません'
  return (
    noteKinds.find((noteKind) => noteKind.key === value)?.display_name ??
    '未登録の種別'
  )
}

export function NoteKindFilterSelect({
  noteKinds,
  catalogState,
  value,
  defaultOpen,
  onChange,
}: NoteKindFilterSelectProps) {
  const selectedKindIsRegistered = noteKinds.some(
    (noteKind) => noteKind.key === value,
  )
  const selectedValue = encodePrefixedSelectValue(
    value,
    KIND_VALUE_PREFIX,
    ALL_KINDS_VALUE,
  )

  return (
    <Select
      value={selectedValue}
      defaultOpen={defaultOpen}
      onValueChange={(selected) => {
        onChange(
          decodePrefixedSelectValue(selected, KIND_VALUE_PREFIX, [
            ALL_KINDS_VALUE,
          ]),
        )
      }}
    >
      <SelectTrigger
        aria-label="種別で絞り込み"
        className="h-auto min-h-11 w-full min-w-0"
      >
        <SelectValue className="whitespace-normal break-words">
          {getSelectedValueLabel(value, noteKinds, catalogState)}
        </SelectValue>
      </SelectTrigger>
      <SelectContent>
        <SelectItem value={ALL_KINDS_VALUE}>{ALL_KINDS_LABEL}</SelectItem>
        {value != null && value !== '' && !selectedKindIsRegistered && (
          <SelectItem
            value={selectedValue}
            className="[&_[data-slot=select-item-text]]:whitespace-normal"
          >
            {getSelectedValueLabel(value, noteKinds, catalogState)}
          </SelectItem>
        )}
        {noteKinds.map((noteKind) => (
          <SelectItem
            key={noteKind.key}
            value={encodePrefixedSelectValue(
              noteKind.key,
              KIND_VALUE_PREFIX,
              ALL_KINDS_VALUE,
            )}
            className="[&_[data-slot=select-item-text]]:whitespace-normal"
          >
            {noteKind.display_name}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}
