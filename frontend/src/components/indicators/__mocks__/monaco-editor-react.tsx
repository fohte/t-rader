// vitest 用に Monaco Editor を textarea に置換するモック。
// プロジェクトの jsdom 環境では実際の Monaco がレンダリングできないため。
import { Textarea } from '@fohte/ui/textarea'
import type { ChangeEvent } from 'react'

interface MockEditorProps {
  value?: string
  onChange?: (value: string | undefined) => void
  options?: { ariaLabel?: string; readOnly?: boolean }
}

export default function Editor({ value, onChange, options }: MockEditorProps) {
  return (
    <Textarea
      aria-label={options?.ariaLabel}
      readOnly={options?.readOnly}
      value={value ?? ''}
      onChange={(e: ChangeEvent<HTMLTextAreaElement>) => {
        onChange?.(e.target.value)
      }}
    />
  )
}
