import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@fohte/ui/select'

const ALL_TAGS_VALUE = '__all-tags__'
const TAG_VALUE_PREFIX = 'tag:'

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
  return (
    <Select
      value={value == null ? ALL_TAGS_VALUE : `${TAG_VALUE_PREFIX}${value}`}
      defaultOpen={defaultOpen}
      onValueChange={(selectedValue) => {
        onChange(
          selectedValue === ALL_TAGS_VALUE
            ? undefined
            : (selectedValue?.slice(TAG_VALUE_PREFIX.length) ?? undefined),
        )
      }}
    >
      <SelectTrigger aria-label="タグで絞り込み">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value={ALL_TAGS_VALUE}>すべてのタグ</SelectItem>
        {tags.map((tag) => (
          <SelectItem key={tag} value={`${TAG_VALUE_PREFIX}${tag}`}>
            {tag}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}
