import { useQueryClient } from '@tanstack/react-query'
import { err, ok, Result, ResultAsync } from 'neverthrow'
import { useEffect, useMemo, useRef, useState } from 'react'

import type {
  IndicatorEditorValue,
  IndicatorScopeLabel,
  PreviewState,
} from '#components/indicators/indicator-editor'
import { IndicatorsPageView } from '#components/indicators/indicators-page-view'
import { $api, fetchClient } from '#lib/api/client'
import { parseJson } from '#lib/json'

interface IndicatorsPageProps {
  scope: IndicatorScopeLabel
  /** 戦略 scope のときのみ指定 */
  strategyId?: string
}

interface IndicatorModel {
  indicator_id: string
  name: string
  scope: string
  strategy_id?: string | null
  code: string
  input_schema: unknown
  output_schema: unknown
  description?: string | null
}

const EMPTY_FORM: IndicatorEditorValue = {
  name: '',
  code: 'def evaluate(args):\n    return {"value": 0}\n\n\nimport json, sys\n\nargs = json.load(sys.stdin)["args"]\nprint(json.dumps(evaluate(args)))\n',
  inputSchema: JSON.stringify({ type: 'object' }, null, 2),
  outputSchema: JSON.stringify({ type: 'object' }, null, 2),
  description: '',
}

function modelToForm(m: IndicatorModel): IndicatorEditorValue {
  return {
    name: m.name,
    code: m.code,
    inputSchema: JSON.stringify(m.input_schema, null, 2),
    outputSchema: JSON.stringify(m.output_schema, null, 2),
    description: m.description ?? '',
  }
}

export function IndicatorsPage({ scope, strategyId }: IndicatorsPageProps) {
  const queryClient = useQueryClient()

  const globalList = $api.useQuery('get', '/api/indicators', undefined, {
    enabled: scope === 'global',
  })
  const strategyList = $api.useQuery(
    'get',
    '/api/strategies/{id}/indicators',
    { params: { path: { id: strategyId ?? '' } } },
    {
      enabled: scope === 'strategy' && strategyId != null && strategyId !== '',
    },
  )

  const list = scope === 'global' ? globalList : strategyList
  const indicators: IndicatorModel[] = list.data ?? []
  const isPending = list.isPending

  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [isCreating, setIsCreating] = useState(false)
  useEffect(() => {
    if (selectedId != null || isCreating) return
    if (indicators.length > 0) {
      const first = indicators[0]
      if (first != null) setSelectedId(first.indicator_id)
    }
  }, [indicators, selectedId, isCreating])

  const selected = useMemo<IndicatorModel | null>(() => {
    if (selectedId == null) return null
    return indicators.find((i) => i.indicator_id === selectedId) ?? null
  }, [indicators, selectedId])

  const initial: IndicatorEditorValue = isCreating
    ? EMPTY_FORM
    : selected != null
      ? modelToForm(selected)
      : EMPTY_FORM

  const createGlobal = $api.useMutation('post', '/api/indicators')
  const createStrategy = $api.useMutation(
    'post',
    '/api/strategies/{id}/indicators',
  )
  const updateMutation = $api.useMutation(
    'put',
    '/api/indicators/{indicator_id}',
  )

  const [saveError, setSaveError] = useState<string | null>(null)
  const [preview, setPreview] = useState<PreviewState>({
    isRunning: false,
    error: null,
    result: null,
  })
  // 進行中の preview リクエストを識別する。indicator 切替や別の preview 発火で値が
  // 進み、await から戻った時に「自分が最新のリクエストか」を判定して stale な結果が
  // 後勝ちで描画されないようにする。
  const previewReqIdRef = useRef(0)

  function invalidateList() {
    if (scope === 'global') {
      void queryClient.invalidateQueries({
        queryKey: $api.queryOptions('get', '/api/indicators').queryKey,
      })
    } else if (strategyId != null) {
      void queryClient.invalidateQueries({
        queryKey: $api.queryOptions('get', '/api/strategies/{id}/indicators', {
          params: { path: { id: strategyId } },
        }).queryKey,
      })
    }
  }

  function parseJsonObject(
    label: string,
    raw: string,
  ): Result<{ [key: string]: unknown }, Error> {
    return parseJson(raw)
      .mapErr(
        (e) =>
          new Error(
            `${label} が JSON として不正です: ${e instanceof Error ? e.message : String(e)}`,
          ),
      )
      .andThen((parsed) => {
        if (
          parsed === null ||
          typeof parsed !== 'object' ||
          Array.isArray(parsed)
        ) {
          return err(new Error(`${label} はオブジェクトである必要があります`))
        }
        // 上のガードで null / 非 object / array は除外済み。
        // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- 直前で plain object と確認済み
        return ok(parsed as { [key: string]: unknown })
      })
  }

  function parseJsonValue(label: string, raw: string): Result<unknown, Error> {
    return parseJson(raw).mapErr(
      (e) =>
        new Error(
          `${label} が JSON として不正です: ${e instanceof Error ? e.message : String(e)}`,
        ),
    )
  }

  function handleSave(next: IndicatorEditorValue) {
    setSaveError(null)
    const inputSchemaResult = parseJsonObject('input_schema', next.inputSchema)
    if (inputSchemaResult.isErr()) {
      setSaveError(inputSchemaResult.error.message)
      return
    }
    const outputSchemaResult = parseJsonObject(
      'output_schema',
      next.outputSchema,
    )
    if (outputSchemaResult.isErr()) {
      setSaveError(outputSchemaResult.error.message)
      return
    }
    const inputSchema = inputSchemaResult.value
    const outputSchema = outputSchemaResult.value

    const body = {
      name: next.name,
      code: next.code,
      input_schema: inputSchema,
      output_schema: outputSchema,
      description: next.description === '' ? null : next.description,
    }

    const onSuccess = (created: IndicatorModel) => {
      // 楽観的に list キャッシュへ追加する。これをせずに invalidate のみだと、
      // 再フェッチが完了するまでの間 `selected` が一時的に null になり、
      // フォームが EMPTY_FORM にフラッシュする。
      const listQueryKey =
        scope === 'global'
          ? $api.queryOptions('get', '/api/indicators').queryKey
          : strategyId != null
            ? $api.queryOptions('get', '/api/strategies/{id}/indicators', {
                params: { path: { id: strategyId } },
              }).queryKey
            : null
      if (listQueryKey != null) {
        queryClient.setQueryData<IndicatorModel[]>(listQueryKey, (old) => [
          ...(old ?? []),
          created,
        ])
      }
      invalidateList()
      setIsCreating(false)
      setSelectedId(created.indicator_id)
    }
    const onError = () => {
      setSaveError('保存に失敗しました')
    }

    if (isCreating) {
      if (scope === 'global') {
        createGlobal.mutate({ body }, { onSuccess, onError })
      } else if (strategyId != null && strategyId !== '') {
        createStrategy.mutate(
          { params: { path: { id: strategyId } }, body },
          { onSuccess, onError },
        )
      } else {
        setSaveError('戦略 scope での作成には strategyId が必要です')
      }
    } else if (selected != null) {
      updateMutation.mutate(
        {
          params: { path: { indicator_id: selected.indicator_id } },
          body,
        },
        {
          onSuccess: () => {
            invalidateList()
          },
          onError,
        },
      )
    }
  }

  async function handlePreview(args: {
    code: string
    inputSchema: string
    outputSchema: string
    argsJson: string
  }) {
    const myReqId = ++previewReqIdRef.current
    setPreview({ isRunning: true, error: null, result: null })

    const inputSchemaResult = parseJsonObject('input_schema', args.inputSchema)
    if (inputSchemaResult.isErr()) {
      if (previewReqIdRef.current !== myReqId) return
      setPreview({
        isRunning: false,
        error: inputSchemaResult.error.message,
        result: null,
      })
      return
    }
    const outputSchemaResult = parseJsonObject(
      'output_schema',
      args.outputSchema,
    )
    if (outputSchemaResult.isErr()) {
      if (previewReqIdRef.current !== myReqId) return
      setPreview({
        isRunning: false,
        error: outputSchemaResult.error.message,
        result: null,
      })
      return
    }
    const argsResult = parseJsonValue('args', args.argsJson)
    if (argsResult.isErr()) {
      if (previewReqIdRef.current !== myReqId) return
      setPreview({
        isRunning: false,
        error: argsResult.error.message,
        result: null,
      })
      return
    }
    const inputSchema = inputSchemaResult.value
    const outputSchema = outputSchemaResult.value
    const parsedArgs = argsResult.value

    // fetch 自体の reject (ネットワーク断・CORS 失敗・abort 等) は
    // openapi-fetch の { data, error } 経路に乗らず throw されるため、
    // ResultAsync.fromPromise で拾わないと「実行中…」表示のまま固まる。
    const responseResult = await ResultAsync.fromPromise(
      fetchClient.POST('/api/indicators/preview', {
        body: {
          code: args.code,
          input_schema: inputSchema,
          output_schema: outputSchema,
          args: parsedArgs,
        },
      }),
      (e) => (e instanceof Error ? e.message : String(e)),
    )
    if (previewReqIdRef.current !== myReqId) return
    if (responseResult.isErr()) {
      setPreview({
        isRunning: false,
        error: responseResult.error,
        result: null,
      })
      return
    }
    const { data, error } = responseResult.value
    if (error != null) {
      setPreview({ isRunning: false, error: error.error, result: null })
      return
    }
    setPreview({
      isRunning: false,
      error: null,
      result: {
        output: data.output ?? null,
        stdout: data.stdout,
        stderr: data.stderr,
        exit_code: data.exit_code,
      },
    })
  }

  // indicator 選択を切り替える。in-flight な preview を無効化し
  // (handlePreview の myReqId 検査で stale 結果が捨てられる)、表示状態をリセットする。
  function selectIndicator(target: { id: string | null; creating: boolean }) {
    setSelectedId(target.id)
    setIsCreating(target.creating)
    setSaveError(null)
    previewReqIdRef.current += 1
    setPreview({ isRunning: false, error: null, result: null })
  }

  const isSaving =
    createGlobal.isPending ||
    createStrategy.isPending ||
    updateMutation.isPending

  return (
    <IndicatorsPageView
      scope={scope}
      indicators={indicators.map((indicator) => ({
        id: indicator.indicator_id,
        name: indicator.name,
      }))}
      isPending={isPending}
      selectedId={selectedId}
      isCreating={isCreating}
      initial={initial}
      isSaving={isSaving}
      saveError={saveError}
      onSelectIndicator={selectIndicator}
      onSave={handleSave}
      onPreview={(p) => {
        void handlePreview(p)
      }}
      preview={preview}
    />
  )
}
