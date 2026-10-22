import { Button } from '@fohte/ui/button'
import { Plus, Upload } from 'lucide-react'

export function TradesPageHeader({
  onImport,
  onAdd,
}: {
  onImport: () => void
  onAdd: () => void
}) {
  return (
    <header className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
      <div className="min-w-0 flex-1">
        <h1 className="mb-1.5 text-2xl font-bold leading-tight tracking-tight">
          取引履歴
        </h1>
        <p className="max-w-180 text-sm leading-relaxed text-muted-foreground-strong">
          全戦略横断の約定記録。入力ソースは証券会社 API / CSV 取込 /
          手入力に非依存のモデル。成績・保有はここから導出します。
        </p>
      </div>
      <div className="flex flex-shrink-0 gap-2">
        <Button type="button" variant="outline" onClick={onImport}>
          <Upload />
          SBI CSV 取込
        </Button>
        <Button type="button" onClick={onAdd}>
          <Plus />
          取引を追加
        </Button>
      </div>
    </header>
  )
}
