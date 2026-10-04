interface NoteTagFilterSelectProps {
  tags: string[]
  value: string | undefined
  onChange: (value: string | undefined) => void
}

export function NoteTagFilterSelect({
  tags,
  value,
  onChange,
}: NoteTagFilterSelectProps) {
  return (
    <select
      aria-label="タグで絞り込み"
      value={value == null ? '' : `tag:${value}`}
      onChange={(event) => {
        const selectedValue = event.target.value
        onChange(
          selectedValue === '' ? undefined : selectedValue.slice('tag:'.length),
        )
      }}
      className="h-9 border border-input bg-transparent px-3 font-mono text-xs"
    >
      <option value="">すべてのタグ</option>
      {tags.map((tag) => (
        <option key={tag} value={`tag:${tag}`}>
          {tag}
        </option>
      ))}
    </select>
  )
}
