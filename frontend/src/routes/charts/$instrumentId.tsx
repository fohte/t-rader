import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/charts/$instrumentId')({
  component: ChartPage,
})

function ChartPage() {
  const { instrumentId } = Route.useParams()

  return (
    <div>
      <h1 className="text-2xl font-bold">チャート: {instrumentId}</h1>
      <p className="mt-2 text-muted-foreground">
        銘柄 {instrumentId} のチャートを表示します
      </p>
    </div>
  )
}
