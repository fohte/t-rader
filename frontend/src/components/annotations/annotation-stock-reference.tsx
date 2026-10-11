import { Link } from '@tanstack/react-router'

export function AnnotationStockReference({
  name,
  symbol,
}: {
  name: string | null
  symbol: string
}) {
  return (
    <>
      {name == null ? (
        <span className="italic text-muted-foreground">未解決</span>
      ) : (
        <Link
          to="/charts/$instrumentId"
          params={{ instrumentId: symbol }}
          className="text-foreground hover:underline"
        >
          {name}
        </Link>
      )}
      <span className="ml-2 text-muted-foreground">{symbol}</span>
    </>
  )
}
