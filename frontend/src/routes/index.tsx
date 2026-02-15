import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/')({
  component: WatchlistPage,
})

function WatchlistPage() {
  return (
    <div>
      <h1 className="text-2xl font-bold">ウォッチリスト</h1>
      <p className="mt-2 text-muted-foreground">
        銘柄を追加して株価チャートを確認できます
      </p>
    </div>
  )
}
