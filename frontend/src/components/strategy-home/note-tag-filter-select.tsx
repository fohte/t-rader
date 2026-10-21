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

const ALL_TAGS_VALUE = '__all-tags__'
const TAG_VALUE_PREFIX = 'tag:'
const ALL_TAGS_LABEL = 'すべてのタグ'

interface NoteTagFilterSelectProps {
  tags: string[]
  value: string | undefined
  defaultOpen?: boolean
  onChange: (value: string | undefined) => void
}

export function NoteTagFilterSelect({
  tags,
  value,
  defaultOpen,
  onChange,
}: NoteTagFilterSelectProps) {
  const selectedTagLabel =
    value == null || value === '' ? ALL_TAGS_LABEL : value

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
      <SelectTrigger aria-label="タグで絞り込み">
        <SelectValue>{selectedTagLabel}</SelectValue>
      </SelectTrigger>
      <SelectContent>
        <SelectItem value={ALL_TAGS_VALUE}>{ALL_TAGS_LABEL}</SelectItem>
        {tags.map((tag) => (
          <SelectItem
            key={tag}
            value={encodePrefixedSelectValue(
              tag,
              TAG_VALUE_PREFIX,
              ALL_TAGS_VALUE,
            )}
          >
            {tag}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}
