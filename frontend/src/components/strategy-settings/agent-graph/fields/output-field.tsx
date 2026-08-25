import { useRef, useState } from 'react'
import { stringify } from 'yaml'

import { CodeEditor } from '#components/indicators/code-editor'
import { checkOutputSchemaText } from '#components/strategy-settings/agent-graph/output-schema-check'

interface OutputFieldProps {
  value: Record<string, unknown>
  onChange: (next: Record<string, unknown>) => void
}

function stringifyOutput(output: Record<string, unknown>): string {
  return Object.keys(output).length === 0 ? '' : stringify(output)
}

export function OutputField({ value, onChange }: OutputFieldProps) {
  const [rawText, setRawText] = useState(() => stringifyOutput(value))
  // 直近で自分が onChange に渡した output。これと異なる value が来たら
  // YAML ビュー編集やフェーズ切り替えなど外部由来の変更とみなし rawText を resync する。
  // `value` は親が毎レンダー parseAgentGraphPhases() で作り直すため参照比較はできない。
  const lastCommittedRef = useRef(JSON.stringify(value))

  const valueJson = JSON.stringify(value)
  if (valueJson !== lastCommittedRef.current) {
    lastCommittedRef.current = valueJson
    setRawText(stringifyOutput(value))
  }

  const check = checkOutputSchemaText(rawText)
  const firstIssue = check.issues[0]

  function handleChange(text: string) {
    setRawText(text)
    const result = checkOutputSchemaText(text)
    if (result.output != null) {
      lastCommittedRef.current = JSON.stringify(result.output)
      onChange(result.output)
    }
  }

  return (
    <>
      <span className="pt-1.5 font-mono text-2xs text-muted-foreground">
        出力スキーマ
      </span>
      <div className="border border-border">
        <div className="flex items-center gap-2 border-b border-border bg-bg-tertiary px-2 py-1 font-mono text-[10.5px] text-muted-foreground">
          <span>output (JSON Schema)</span>
          <span
            className={
              firstIssue != null
                ? 'ml-auto text-primary'
                : 'ml-auto text-emerald-400'
            }
          >
            {firstIssue != null
              ? `✗ ${String(firstIssue.line)} 行目: ${firstIssue.message}`
              : '✓ valid'}
          </span>
        </div>
        <CodeEditor
          language="yaml"
          value={rawText}
          onChange={handleChange}
          ariaLabel="出力スキーマ"
          height={160}
        />
      </div>
    </>
  )
}
