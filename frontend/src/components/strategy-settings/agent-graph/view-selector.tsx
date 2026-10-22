import { SegmentedControl } from '@fohte/ui/segmented-control'

export type AgentGraphView = 'form' | 'yaml'

const OPTIONS = [
  { value: 'form', label: 'フォーム' },
  { value: 'yaml', label: 'YAML' },
] as const

interface AgentGraphViewSelectorProps {
  value: AgentGraphView
  formAvailable: boolean
  onChange: (value: AgentGraphView) => void
}

export function AgentGraphViewSelector({
  value,
  formAvailable,
  onChange,
}: AgentGraphViewSelectorProps) {
  return (
    <div className="font-mono">
      <SegmentedControl
        aria-label="表示形式"
        value={value}
        options={formAvailable ? OPTIONS : [OPTIONS[1]]}
        onValueChange={onChange}
      />
    </div>
  )
}
