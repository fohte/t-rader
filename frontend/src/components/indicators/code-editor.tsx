import '#components/indicators/monaco-setup'

import Editor from '@monaco-editor/react'

interface CodeEditorProps {
  language: 'python' | 'json' | 'yaml'
  value: string
  onChange: (next: string) => void
  /** test 環境では Monaco を mock 済み textarea 風に差し替える。data-testid を渡す用。 */
  testId?: string
  height?: number
  ariaLabel?: string
  readOnly?: boolean
}

export function CodeEditor({
  language,
  value,
  onChange,
  testId,
  height = 320,
  ariaLabel,
  readOnly = false,
}: CodeEditorProps) {
  return (
    <div
      data-testid={testId}
      className="border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-secondary)]"
      style={{ height: `${String(height)}px` }}
    >
      <Editor
        height="100%"
        language={language}
        value={value}
        onChange={(next) => {
          onChange(next ?? '')
        }}
        theme="vs-dark"
        options={{
          readOnly,
          minimap: { enabled: false },
          fontSize: 13,
          scrollBeyondLastLine: false,
          tabSize: 2,
          ariaLabel,
        }}
      />
    </div>
  )
}
