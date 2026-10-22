import { Result } from 'neverthrow'
import { useRef, useState } from 'react'
import { parse as parseYaml, stringify } from 'yaml'

import { isRecord } from '#components/strategy-settings/agent-graph/document'
import { InputBindingsFieldView } from '#components/strategy-settings/agent-graph/fields/input-bindings-field-view'

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
    <InputBindingsFieldView
      value={rawText}
      invalidInput={invalidInput}
      onChange={handleChange}
    />
  )
}
