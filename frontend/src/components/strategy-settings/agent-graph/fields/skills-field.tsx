import { Chip } from '@fohte/ui/chip'

import { ChipList } from '#components/strategy-settings/agent-graph/fields/chip-list'

interface SkillsFieldProps {
  value: string[]
  onChange: (next: string[]) => void
  options: string[]
}

export function SkillsField({ value, onChange, options }: SkillsFieldProps) {
  return (
    <>
      <Chip>skills</Chip>
      <ChipList
        values={value}
        options={options}
        onAdd={(name) => {
          onChange([...value, name])
        }}
        onRemove={(name) => {
          onChange(value.filter((v) => v !== name))
        }}
        removeAriaLabel={(name) => `skill "${name}" を外す`}
        addAriaLabel="skill を追加"
      />
    </>
  )
}
