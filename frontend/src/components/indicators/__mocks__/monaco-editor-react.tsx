// vitest 用に Monaco Editor を textarea に置換するモック。
// プロジェクトの jsdom 環境では実際の Monaco がレンダリングできないため。
import type { ChangeEvent } from 'react'

interface MockEditorProps {
  value?: string
  onChange?: (value: string | undefined) => void
  options?: { ariaLabel?: string; readOnly?: boolean }
}

export default function Editor({ value, onChange, options }: MockEditorProps) {
  return (
    <textarea
      aria-label={options?.ariaLabel}
      readOnly={options?.readOnly}
      value={value ?? ''}
      onChange={(e: ChangeEvent<HTMLTextAreaElement>) => {
        onChange?.(e.target.value)
      }}
    />
  )
}
