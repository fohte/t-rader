import { Chip } from '@fohte/ui/chip'

import { CodeEditor } from '#components/indicators/code-editor'

interface InputBindingsFieldViewProps {
  value: string
  invalidInput: boolean
  onChange: (next: string) => void
}

export function InputBindingsFieldView({
  value,
  invalidInput,
  onChange,
}: InputBindingsFieldViewProps) {
  return (
    <>
      <Chip>入力 binding</Chip>
      <div className="border border-border">
        <div className="flex items-center gap-2 border-b border-border bg-bg-tertiary px-2 py-1 font-mono text-2xs text-muted-foreground">
          <Chip>input (YAML)</Chip>
          <span className="ml-auto text-muted-foreground">
            {invalidInput
              ? 'mapping を確認してください'
              : 'phase 入力の割り当て'}
          </span>
        </div>
        <CodeEditor
          language="yaml"
          value={value}
          onChange={onChange}
          ariaLabel="入力 binding"
          height={140}
        />
      </div>
    </>
  )
}
