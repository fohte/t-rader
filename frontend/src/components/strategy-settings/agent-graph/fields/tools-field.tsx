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
      <span className="pt-1.5 font-mono text-2xs text-muted-foreground">
        使える tool
      </span>
      {value === undefined ? (
        <div className="flex items-center gap-2">
          <span className="border border-dashed border-border px-1.5 py-0.5 font-mono text-2xs text-muted-foreground">
            すべての tool
          </span>
          <button
            type="button"
            onClick={() => {
              onChange([])
            }}
            className="font-mono text-2xs text-muted-foreground underline hover:text-foreground"
          >
            絞り込む
          </button>
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
            <button
              type="button"
              onClick={() => {
                onChange(undefined)
              }}
              className="font-mono text-2xs text-muted-foreground underline hover:text-foreground"
            >
              全 tool に戻す
            </button>
          }
        />
      )}
    </>
  )
}
