import { Button } from '@fohte/ui/button'
import { Input } from '@fohte/ui/input'

import { Skeleton } from '#components/ui/skeleton'
import type {
  GroupRatioDraft,
  GroupRatioDraftErrors,
} from '#lib/group-ratio-policy'

export type { GroupRatioDraft } from '#lib/group-ratio-policy'

interface GroupRatioAxisOption {
  key: string
  name: string
}

export interface GroupRatioEditorProps {
  axes: GroupRatioAxisOption[]
  rows: GroupRatioDraft[]
  errors: Array<GroupRatioDraftErrors | null>
  isLoading: boolean
  loadError: boolean
  isSaving: boolean
  saveError: string | null
  onRowsChange: (rows: GroupRatioDraft[]) => void
  onSave: () => void
}

export function GroupRatioEditor({
  axes,
  rows,
  errors,
  isLoading,
  loadError,
  isSaving,
  saveError,
  onRowsChange,
  onSave,
}: GroupRatioEditorProps) {
  if (isLoading) return <Skeleton className="h-48 w-full max-w-xl" />
  if (loadError) {
    return (
      <p className="font-mono text-xs text-primary">
        リスク上限の読み込みに失敗しました
      </p>
    )
  }

  const hasAvailableAxis = axes.some(
    (axis) => !rows.some((row) => row.axis === axis.key),
  )

  function updateRow(index: number, patch: Partial<GroupRatioDraft>) {
    onRowsChange(
      rows.map((row, rowIndex) =>
        rowIndex === index ? { ...row, ...patch } : row,
      ),
    )
  }

  return (
    <div className="max-w-xl space-y-4">
      {rows.length === 0 ? (
        <p className="font-mono text-xs text-muted-foreground-strong">
          上限は設定されていません
        </p>
      ) : (
        <div className="space-y-3">
          {rows.map((row, index) => (
            <div
              key={`${row.axis}-${String(index)}`}
              className="grid gap-2 border border-border p-3 sm:grid-cols-3 sm:items-end"
            >
              <div className="space-y-1.5">
                <label
                  htmlFor={`group-ratio-axis-${String(index)}`}
                  className="block font-mono text-2xs uppercase tracking-wider text-muted-foreground"
                >
                  分類軸
                </label>
                <select
                  id={`group-ratio-axis-${String(index)}`}
                  value={row.axis}
                  disabled={isSaving}
                  onChange={(event) => {
                    updateRow(index, { axis: event.target.value })
                  }}
                  className="h-9 w-full border border-border bg-background px-2 text-sm text-foreground outline-none disabled:opacity-60"
                >
                  <option value="">分類軸を選択</option>
                  {row.axis !== '' &&
                    !axes.some((axis) => axis.key === row.axis) && (
                      <option value={row.axis}>
                        {row.axis} (現在利用できません)
                      </option>
                    )}
                  {axes.map((axis) => {
                    const isUsedElsewhere = rows.some(
                      (otherRow, otherIndex) =>
                        otherIndex !== index && otherRow.axis === axis.key,
                    )
                    return (
                      <option
                        key={axis.key}
                        value={axis.key}
                        disabled={isUsedElsewhere}
                      >
                        {axis.name}
                      </option>
                    )
                  })}
                </select>
                {errors[index]?.axis != null && (
                  <p className="font-mono text-2xs text-primary">
                    {errors[index].axis}
                  </p>
                )}
              </div>
              <div className="space-y-1.5">
                <label
                  htmlFor={`group-ratio-value-${String(index)}`}
                  className="block font-mono text-2xs uppercase tracking-wider text-muted-foreground"
                >
                  保有時価上限 (口座全体に対する割合)
                </label>
                <div className="flex items-center gap-2">
                  <Input
                    id={`group-ratio-value-${String(index)}`}
                    inputMode="decimal"
                    value={row.ratioPercent}
                    placeholder="上限比率"
                    aria-invalid={errors[index]?.ratioPercent != null}
                    disabled={isSaving}
                    onChange={(event) => {
                      updateRow(index, { ratioPercent: event.target.value })
                    }}
                  />
                  <span className="font-mono text-xs text-muted-foreground">
                    %
                  </span>
                </div>
                {errors[index]?.ratioPercent != null && (
                  <p className="font-mono text-2xs text-primary">
                    {errors[index].ratioPercent}
                  </p>
                )}
              </div>
              <Button
                type="button"
                variant="outline"
                disabled={isSaving}
                onClick={() => {
                  onRowsChange(rows.filter((_, rowIndex) => rowIndex !== index))
                }}
              >
                削除
              </Button>
            </div>
          ))}
        </div>
      )}
      <div className="flex flex-wrap items-center gap-3">
        <Button
          type="button"
          variant="outline"
          disabled={isSaving || !hasAvailableAxis}
          onClick={() => {
            onRowsChange([...rows, { axis: '', ratioPercent: '' }])
          }}
        >
          分類軸を追加
        </Button>
        <Button type="button" onClick={onSave} disabled={isSaving}>
          {isSaving ? '保存中…' : '保存'}
        </Button>
        {saveError != null && (
          <span
            data-testid="save-error"
            className="font-mono text-xs text-primary"
          >
            {saveError}
          </span>
        )}
      </div>
    </div>
  )
}
