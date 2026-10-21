import { Button } from '@fohte/ui/button'
import { Chip } from '@fohte/ui/chip'
import { Input } from '@fohte/ui/input'

import { CodeEditor } from '#components/indicators/code-editor'
import type {
  IndicatorEditorValue,
  IndicatorScopeLabel,
  PreviewState,
} from '#components/indicators/indicator-editor'

interface IndicatorEditorViewProps {
  scope: IndicatorScopeLabel
  value: IndicatorEditorValue
  isDirty: boolean
  nameReadOnly: boolean
  onPatch: (patch: Partial<IndicatorEditorValue>) => void
  argsJson: string
  onArgsJsonChange: (next: string) => void
  onSave: () => void
  isSaving: boolean
  saveError: string | null
  onPreview: () => void
  preview: PreviewState
}

export function IndicatorEditorView({
  scope,
  value,
  isDirty,
  nameReadOnly,
  onPatch,
  argsJson,
  onArgsJsonChange,
  onSave,
  isSaving,
  saveError,
  onPreview,
  preview,
}: IndicatorEditorViewProps) {
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-end gap-3">
        <div className="space-y-1 font-mono text-sm">
          <label
            htmlFor="indicator-name"
            className="text-2xs uppercase tracking-wider text-muted-foreground"
          >
            name
          </label>
          <Input
            id="indicator-name"
            value={value.name}
            readOnly={nameReadOnly}
            onChange={(e) => {
              onPatch({ name: e.target.value })
            }}
            className="w-64"
          />
        </div>
        <div
          data-testid="indicator-scope"
          className="font-mono text-2xs uppercase tracking-wider text-muted-foreground"
        >
          scope: {scope}
        </div>
        {isDirty && (
          <Chip data-testid="dirty-indicator" size="sm" tone="strong">
            未保存の変更あり
          </Chip>
        )}
      </div>

      <div className="space-y-1.5 font-mono text-sm">
        <label
          htmlFor="indicator-description"
          className="text-2xs uppercase tracking-wider text-muted-foreground"
        >
          description
        </label>
        <Input
          id="indicator-description"
          value={value.description}
          onChange={(e) => {
            onPatch({ description: e.target.value })
          }}
          className="w-full"
        />
      </div>

      <div className="space-y-1.5">
        <div className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
          code (Python)
        </div>
        <CodeEditor
          language="python"
          value={value.code}
          onChange={(next) => {
            onPatch({ code: next })
          }}
          testId="indicator-code-editor"
          ariaLabel="indicator code"
          height={360}
        />
      </div>

      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
        <div className="space-y-1.5">
          <div className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
            input_schema (JSON Schema)
          </div>
          <CodeEditor
            language="json"
            value={value.inputSchema}
            onChange={(next) => {
              onPatch({ inputSchema: next })
            }}
            testId="indicator-input-schema-editor"
            ariaLabel="input_schema"
            height={200}
          />
        </div>
        <div className="space-y-1.5">
          <div className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
            output_schema (JSON Schema)
          </div>
          <CodeEditor
            language="json"
            value={value.outputSchema}
            onChange={(next) => {
              onPatch({ outputSchema: next })
            }}
            testId="indicator-output-schema-editor"
            ariaLabel="output_schema"
            height={200}
          />
        </div>
      </div>

      <div className="flex items-center gap-3">
        <Button type="button" onClick={onSave} disabled={!isDirty || isSaving}>
          {isSaving ? '保存中…' : '保存'}
        </Button>
        {saveError != null && (
          <span className="font-mono text-xs text-destructive">
            {saveError}
          </span>
        )}
      </div>

      <div className="space-y-2 border-t border-border pt-4">
        <div className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
          preview
        </div>
        <div className="space-y-1.5">
          <label
            htmlFor="indicator-args"
            className="font-mono text-2xs text-muted-foreground"
          >
            args (JSON)
          </label>
          <CodeEditor
            language="json"
            value={argsJson}
            onChange={onArgsJsonChange}
            testId="indicator-args-editor"
            ariaLabel="preview args"
            height={120}
          />
        </div>
        <div className="flex items-center gap-3">
          <Button
            type="button"
            onClick={onPreview}
            disabled={preview.isRunning}
          >
            {preview.isRunning ? '実行中…' : 'プレビュー実行'}
          </Button>
          {preview.error != null && (
            <span
              data-testid="preview-error"
              className="font-mono text-xs text-destructive"
            >
              {preview.error}
            </span>
          )}
        </div>

        {preview.result != null && (
          <div
            data-testid="preview-result"
            className="space-y-2 border border-border bg-bg-secondary p-3"
          >
            <div className="flex items-center gap-4 font-mono text-2xs uppercase tracking-wider text-muted-foreground">
              <span>
                exit_code:{' '}
                <span
                  data-testid="preview-exit-code"
                  className="text-foreground"
                >
                  {preview.result.exit_code}
                </span>
              </span>
            </div>
            {preview.result.output !== null && (
              <div>
                <div className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
                  output
                </div>
                <pre
                  data-testid="preview-output"
                  className="overflow-auto whitespace-pre-wrap font-mono text-xs text-foreground"
                >
                  {JSON.stringify(preview.result.output, null, 2)}
                </pre>
              </div>
            )}
            {preview.result.stdout !== '' && (
              <div>
                <div className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
                  stdout
                </div>
                <pre
                  data-testid="preview-stdout"
                  className="overflow-auto whitespace-pre-wrap font-mono text-xs text-foreground"
                >
                  {preview.result.stdout}
                </pre>
              </div>
            )}
            {preview.result.stderr !== '' && (
              <div>
                <div className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
                  stderr
                </div>
                <pre
                  data-testid="preview-stderr"
                  className="overflow-auto whitespace-pre-wrap font-mono text-xs text-destructive"
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
