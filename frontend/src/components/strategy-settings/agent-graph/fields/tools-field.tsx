import { Button } from '@fohte/ui/button'
import { Chip } from '@fohte/ui/chip'

import { ChipList } from '#components/strategy-settings/agent-graph/fields/chip-list'
import type { components } from '#lib/api/schema.gen'

type AgentTool = components['schemas']['AgentTool']

interface ToolsFieldProps {
  value: string[] | undefined
  onChange: (next: string[] | undefined) => void
  options: AgentTool[]
}

export function ToolsField({ value, onChange, options }: ToolsFieldProps) {
  return (
    <>
      <Chip>使える tool</Chip>
      {value === undefined ? (
        <div className="flex items-center gap-2 font-mono text-2xs">
          <Chip>すべての tool</Chip>
          <Button
            type="button"
            variant="plain"
            onClick={() => {
              onChange([])
            }}
          >
            絞り込む
          </Button>
        </div>
      ) : (
        <ChipList
          values={value}
          options={options.map((t) => t.name)}
          onAdd={(name) => {
            onChange([...value, name])
          }}
          onRemove={(name) => {
            onChange(value.filter((v) => v !== name))
          }}
          removeAriaLabel={(name) => `tool "${name}" を外す`}
          addAriaLabel="tool を追加"
          extra={
            <Button
              type="button"
              variant="plain"
              onClick={() => {
                onChange(undefined)
              }}
            >
              全 tool に戻す
            </Button>
          }
        />
      )}
    </>
  )
}
