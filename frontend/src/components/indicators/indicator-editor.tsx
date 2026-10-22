import { useEffect, useRef, useState } from 'react'

import { IndicatorEditorView } from '#components/indicators/indicator-editor-view'

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
  const lastIsSavingRef = useRef<boolean>(isSaving)
  const dirty = !valuesEqual(value, initial)

  useEffect(() => {
    const wasSaving = lastIsSavingRef.current
    lastIsSavingRef.current = isSaving

    // 保存が `isSaving: true → false` に遷移し、かつエラーがなければ強制同期する。
    // 通常パスでは dirty=true のため initial への追従はスキップされるが、保存直後は
    // サーバー側で正規化された値 (updated_at 等) を反映したく、dirty 回避が必要。
    if (wasSaving && !isSaving && saveError == null) {
      setValue(initial)
      lastInitialRef.current = initial
      return
    }

    if (valuesEqual(value, lastInitialRef.current)) {
      setValue(initial)
    }
    lastInitialRef.current = initial
  }, [initial, value, isSaving, saveError])

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
    <IndicatorEditorView
      scope={scope}
      value={value}
      isDirty={dirty}
      nameReadOnly={nameReadOnly}
      onPatch={patch}
      argsJson={argsJson}
      onArgsJsonChange={setArgsJson}
      onSave={handleSave}
      isSaving={isSaving}
      saveError={saveError}
      onPreview={handlePreview}
      preview={preview}
    />
  )
}
