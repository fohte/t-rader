import { useEffect, useRef, useState } from 'react'

import { CodeEditor } from '@/components/indicators/code-editor'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'

export interface IndicatorEditorValue {
  name: string
  code: string
  inputSchema: string
  outputSchema: string
  description: string
}

export type IndicatorScopeLabel = 'global' | 'strategy'

export interface PreviewState {
  isRunning: boolean
  error: string | null
  result: {
    output: unknown
    stdout: string
    stderr: string
    exit_code: number
  } | null
}

interface IndicatorEditorProps {
  scope: IndicatorScopeLabel
  /** 永続化されている現在の状態。これと現在のフォーム値の diff で dirty を判定する */
  initial: IndicatorEditorValue
  /** name 編集を許可しない (= edit モード) */
  nameReadOnly?: boolean
  onSave: (next: IndicatorEditorValue) => void
  isSaving?: boolean
  saveError?: string | null
  /**
   * プレビューを実行する。args は JSON テキストとして渡す (パースは呼び出し側に任せる)
   */
  onPreview: (args: {
    code: string
    inputSchema: string
    outputSchema: string
    argsJson: string
  }) => void
  preview: PreviewState
}

function valuesEqual(a: IndicatorEditorValue, b: IndicatorEditorValue) {
  return (
    a.name === b.name &&
    a.code === b.code &&
    a.inputSchema === b.inputSchema &&
    a.outputSchema === b.outputSchema &&
    a.description === b.description
  )
}

export function IndicatorEditor({
  scope,
  initial,
  nameReadOnly = false,
  onSave,
  isSaving = false,
  saveError = null,
  onPreview,
  preview,
}: IndicatorEditorProps) {
  const [value, setValue] = useState<IndicatorEditorValue>(initial)
  const [argsJson, setArgsJson] = useState<string>('{}')

  const lastInitialRef = useRef<IndicatorEditorValue>(initial)
  const dirty = !valuesEqual(value, initial)

  useEffect(() => {
    if (valuesEqual(value, lastInitialRef.current)) {
      setValue(initial)
    }
    lastInitialRef.current = initial
  }, [initial, value])

  useEffect(() => {
    if (!dirty) return
    function handler(e: BeforeUnloadEvent) {
      e.preventDefault()
    }
    window.addEventListener('beforeunload', handler)
    return () => {
      window.removeEventListener('beforeunload', handler)
    }
  }, [dirty])

  function patch(p: Partial<IndicatorEditorValue>) {
    setValue((v) => ({ ...v, ...p }))
  }

  function handleSave() {
    if (!dirty || isSaving) return
    onSave(value)
  }

  function handlePreview() {
    onPreview({
      code: value.code,
      inputSchema: value.inputSchema,
      outputSchema: value.outputSchema,
      argsJson,
    })
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-end gap-3">
        <div className="space-y-1">
          <label
            htmlFor="indicator-name"
            className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]"
          >
            name
          </label>
          <Input
            id="indicator-name"
            value={value.name}
            readOnly={nameReadOnly}
            onChange={(e) => {
              patch({ name: e.target.value })
            }}
            className="w-64 font-mono text-[13px]"
          />
        </div>
        <div
          data-testid="indicator-scope"
          className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]"
        >
          scope: {scope}
        </div>
        {dirty && (
          <span
            data-testid="dirty-indicator"
            className="font-mono text-[11px] text-[color:var(--color-accent-strategy)]"
          >
            未保存の変更あり
          </span>
        )}
      </div>

      <div className="space-y-1.5">
        <label
          htmlFor="indicator-description"
          className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]"
        >
          description
        </label>
        <Input
          id="indicator-description"
          value={value.description}
          onChange={(e) => {
            patch({ description: e.target.value })
          }}
          className="w-full font-mono text-[13px]"
        />
      </div>

      <div className="space-y-1.5">
        <div className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
          code (Python)
        </div>
        <CodeEditor
          language="python"
          value={value.code}
          onChange={(next) => {
            patch({ code: next })
          }}
          testId="indicator-code-editor"
          ariaLabel="indicator code"
          height={360}
        />
      </div>

      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
        <div className="space-y-1.5">
          <div className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
            input_schema (JSON Schema)
          </div>
          <CodeEditor
            language="json"
            value={value.inputSchema}
            onChange={(next) => {
              patch({ inputSchema: next })
            }}
            testId="indicator-input-schema-editor"
            ariaLabel="input_schema"
            height={200}
          />
        </div>
        <div className="space-y-1.5">
          <div className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
            output_schema (JSON Schema)
          </div>
          <CodeEditor
            language="json"
            value={value.outputSchema}
            onChange={(next) => {
              patch({ outputSchema: next })
            }}
            testId="indicator-output-schema-editor"
            ariaLabel="output_schema"
            height={200}
          />
        </div>
      </div>

      <div className="flex items-center gap-3">
        <Button
          type="button"
          onClick={handleSave}
          disabled={!dirty || isSaving}
        >
          {isSaving ? '保存中…' : '保存'}
        </Button>
        {saveError != null && (
          <span className="font-mono text-[12px] text-[color:var(--color-accent-strategy)]">
            {saveError}
          </span>
        )}
      </div>

      <div className="space-y-2 border-t border-[color:var(--color-hairline)] pt-4">
        <div className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
          preview
        </div>
        <div className="space-y-1.5">
          <label
            htmlFor="indicator-args"
            className="font-mono text-[11px] text-[color:var(--color-text-tertiary)]"
          >
            args (JSON)
          </label>
          <CodeEditor
            language="json"
            value={argsJson}
            onChange={setArgsJson}
            testId="indicator-args-editor"
            ariaLabel="preview args"
            height={120}
          />
        </div>
        <div className="flex items-center gap-3">
          <Button
            type="button"
            onClick={handlePreview}
            disabled={preview.isRunning}
          >
            {preview.isRunning ? '実行中…' : 'プレビュー実行'}
          </Button>
          {preview.error != null && (
            <span
              data-testid="preview-error"
              className="font-mono text-[12px] text-[color:var(--color-accent-strategy)]"
            >
              {preview.error}
            </span>
          )}
        </div>

        {preview.result != null && (
          <div
            data-testid="preview-result"
            className="space-y-2 border border-[color:var(--color-hairline)] bg-[color:var(--color-bg-secondary)] p-3"
          >
            <div className="flex items-center gap-4 font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
              <span>
                exit_code:{' '}
                <span
                  data-testid="preview-exit-code"
                  className="text-[color:var(--color-text-primary)]"
                >
                  {preview.result.exit_code}
                </span>
              </span>
            </div>
            {preview.result.output !== null && (
              <div>
                <div className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
                  output
                </div>
                <pre
                  data-testid="preview-output"
                  className="overflow-auto whitespace-pre-wrap font-mono text-[12px] text-[color:var(--color-text-primary)]"
                >
                  {JSON.stringify(preview.result.output, null, 2)}
                </pre>
              </div>
            )}
            {preview.result.stdout !== '' && (
              <div>
                <div className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
                  stdout
                </div>
                <pre
                  data-testid="preview-stdout"
                  className="overflow-auto whitespace-pre-wrap font-mono text-[12px] text-[color:var(--color-text-primary)]"
                >
                  {preview.result.stdout}
                </pre>
              </div>
            )}
            {preview.result.stderr !== '' && (
              <div>
                <div className="font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
                  stderr
                </div>
                <pre
                  data-testid="preview-stderr"
                  className="overflow-auto whitespace-pre-wrap font-mono text-[12px] text-[color:var(--color-accent-strategy)]"
                >
                  {preview.result.stderr}
                </pre>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  )
}
