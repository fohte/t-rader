import { Chip } from '@fohte/ui/chip'
import { Textarea } from '@fohte/ui/textarea'

interface PromptFieldProps {
  value: string
  onChange: (next: string) => void
}

export function PromptField({ value, onChange }: PromptFieldProps) {
  return (
    <>
      <Chip>プロンプト</Chip>
      <div className="font-mono">
        <Textarea
          aria-label="プロンプト"
          value={value}
          onChange={(e) => {
            onChange(e.target.value)
          }}
        />
      </div>
    </>
  )
}
