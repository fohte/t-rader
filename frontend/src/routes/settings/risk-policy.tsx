import { useQueryClient } from '@tanstack/react-query'
import { createFileRoute, Link } from '@tanstack/react-router'
import { useEffect, useRef, useState } from 'react'

import { GroupRatioEditor } from '#components/group-ratio-editor'
import { $api } from '#lib/api/client'
import {
  type GroupRatioDraft,
  type GroupRatioDraftErrors,
  parseGroupRatioDrafts,
} from '#lib/group-ratio-policy'
import { formatRatioPercent } from '#lib/ratio-percent'

export const Route = createFileRoute('/settings/risk-policy')({
  component: RiskPolicySettingsPage,
})

function RiskPolicySettingsPage() {
  const queryClient = useQueryClient()
  const {
    data: policy,
    isPending: isPolicyPending,
    isError: isPolicyError,
  } = $api.useQuery('get', '/api/account/risk-policy')
  const {
    data: axes = [],
    isPending: areAxesPending,
    isError: areAxesError,
  } = $api.useQuery('get', '/api/group-axes')
  const mutation = $api.useMutation('put', '/api/account/risk-policy')

  const initialRows = (policy?.max_group_ratios ?? []).map(
    ({ axis, ratio }) => ({
      axis,
      ratioPercent: formatRatioPercent(ratio),
    }),
  )
  const initialRowsJson = JSON.stringify(initialRows)
  const [rows, setRows] = useState<GroupRatioDraft[]>(initialRows)
  const lastInitialRowsRef = useRef(initialRowsJson)
  const [validationErrors, setValidationErrors] = useState<
    Array<GroupRatioDraftErrors | null>
  >([])
  const [saveError, setSaveError] = useState<string | null>(null)

  useEffect(() => {
    if (
      JSON.stringify(rows) === lastInitialRowsRef.current &&
      initialRowsJson !== lastInitialRowsRef.current
    ) {
      setRows(initialRows)
    }
    lastInitialRowsRef.current = initialRowsJson
  }, [initialRowsJson, rows])

  function handleRowsChange(nextRows: GroupRatioDraft[]) {
    setRows(nextRows)
    setValidationErrors([])
    setSaveError(null)
  }

  function handleSave() {
    const parsed = parseGroupRatioDrafts(rows)
    if (parsed.errors.some((error) => error != null)) {
      setValidationErrors(parsed.errors)
      return
    }
    setValidationErrors([])
    setSaveError(null)
    mutation.mutate(
      { body: { max_group_ratios: parsed.maxGroupRatios } },
      {
        onSuccess: () => {
          void queryClient.invalidateQueries({
            queryKey: $api.queryOptions('get', '/api/account/risk-policy')
              .queryKey,
          })
        },
        onError: () => {
          setSaveError('保存に失敗しました')
        },
      },
    )
  }

  return (
    <div className="space-y-5">
      <div>
        <Link
          to="/settings"
          className="font-mono text-xs text-muted-foreground hover:text-foreground"
        >
          &lt; 設定に戻る
        </Link>
      </div>
      <header>
        <h1 className="mb-1 text-2xl font-bold leading-tight tracking-tight">
          設定 — リスク上限
        </h1>
        <p className="text-sm text-muted-foreground-strong">
          口座全体の保有銘柄時価合計に対する、分類軸ごとのグループ保有時価合計の上限比率を設定します。
        </p>
      </header>

      <GroupRatioEditor
        axes={axes.map(({ key, name }) => ({ key, name }))}
        rows={rows}
        errors={validationErrors}
        isLoading={isPolicyPending || areAxesPending}
        loadError={isPolicyError || areAxesError}
        isSaving={mutation.isPending}
        saveError={saveError}
        onRowsChange={handleRowsChange}
        onSave={handleSave}
      />
    </div>
  )
}
