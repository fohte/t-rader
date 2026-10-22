import { Chip } from '@fohte/ui/chip'
import { Result } from 'neverthrow'
import { useRef, useState } from 'react'
import { parse as parseYaml, stringify } from 'yaml'

import { CodeEditor } from '#components/indicators/code-editor'
import { isRecord } from '#components/strategy-settings/agent-graph/document'

interface InputBindingsFieldProps {
  value: Record<string, unknown> | undefined
  onChange: (next: Record<string, unknown>) => void
}

function stringifyInput(input: Record<string, unknown> | undefined): string {
  return input == null || Object.keys(input).length === 0
    ? ''
    : stringify(input)
}

const tryParseYaml = Result.fromThrowable((text: string): unknown =>
  parseYaml(text),
)

export function InputBindingsField({
  value,
  onChange,
}: InputBindingsFieldProps) {
  const [rawText, setRawText] = useState(() => stringifyInput(value))
  const lastCommittedRef = useRef(JSON.stringify(value ?? {}))
  const valueJson = JSON.stringify(value ?? {})
  if (valueJson !== lastCommittedRef.current) {
    lastCommittedRef.current = valueJson
    setRawText(stringifyInput(value))
  }

  const parsed = rawText.trim() === '' ? null : tryParseYaml(rawText)
  const invalidInput =
    parsed != null && (parsed.isErr() || !isRecord(parsed.value))

  function handleChange(text: string) {
    setRawText(text)
    if (text.trim() === '') {
      lastCommittedRef.current = '{}'
      onChange({})
      return
    }
    const result = tryParseYaml(text)
    if (result.isOk() && isRecord(result.value)) {
      lastCommittedRef.current = JSON.stringify(result.value)
      onChange(result.value)
    }
  }

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
          value={rawText}
          onChange={handleChange}
          ariaLabel="入力 binding"
          height={140}
        />
      </div>
    </>
  )
}
