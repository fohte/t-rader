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

type NoteTag = components['schemas']['NoteTag']
type NoteTagCatalogState = 'loading' | 'error' | 'loaded'

const ALL_TAGS_VALUE = '__all-tags__'
const TAG_VALUE_PREFIX = 'tag:'
const ALL_TAGS_LABEL = 'すべてのタグ'

interface NoteTagFilterSelectProps {
  noteTags: readonly NoteTag[]
  catalogState: NoteTagCatalogState
  value: string | undefined
  defaultOpen?: boolean
  className?: string
  onChange: (value: string | undefined) => void
}

export function NoteTagFilterSelect({
  noteTags,
  catalogState,
  value,
  defaultOpen,
  className,
  onChange,
}: NoteTagFilterSelectProps) {
  const selectedNoteTag = noteTags.find((noteTag) => noteTag.key === value)
  const selectedTagLabel =
    value == null || value === ''
      ? ALL_TAGS_LABEL
      : catalogState === 'loading'
        ? 'タグを読み込み中'
        : catalogState === 'error'
          ? 'タグを取得できません'
          : (selectedNoteTag?.display_name ?? '未登録のタグ')

  return (
    <Select
      value={encodePrefixedSelectValue(value, TAG_VALUE_PREFIX, ALL_TAGS_VALUE)}
      defaultOpen={defaultOpen}
      onValueChange={(selectedValue) => {
        onChange(
          decodePrefixedSelectValue(selectedValue, TAG_VALUE_PREFIX, [
            ALL_TAGS_VALUE,
          ]),
        )
      }}
    >
      <SelectTrigger aria-label="タグで絞り込み" className={className}>
        <SelectValue>{selectedTagLabel}</SelectValue>
      </SelectTrigger>
      <SelectContent>
        <SelectItem value={ALL_TAGS_VALUE}>{ALL_TAGS_LABEL}</SelectItem>
        {noteTags.map((noteTag) => (
          <SelectItem
            key={noteTag.key}
            value={encodePrefixedSelectValue(
              noteTag.key,
              TAG_VALUE_PREFIX,
              ALL_TAGS_VALUE,
            )}
          >
            {noteTag.display_name}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}
