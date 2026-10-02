import { Link } from '@tanstack/react-router'

export function IngestStatusPageHeader() {
  return (
    <div className="space-y-2">
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
          設定 — 取り込み状況
        </h1>
        <p className="text-sm text-muted-foreground-strong">
          取り込みジョブの実行履歴と、データが保存されている最新日を確認します。
        </p>
      </header>
    </div>
  )
}
