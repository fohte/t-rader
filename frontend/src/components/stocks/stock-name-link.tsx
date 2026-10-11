import { Link } from '@tanstack/react-router'

export function StockNameLink({
  name,
  symbol,
  missingName,
}: {
  name: string | null
  symbol: string
  missingName: 'link-symbol' | 'unresolved'
}) {
  const linkText = name ?? (missingName === 'link-symbol' ? symbol : null)
  const showSymbol = name != null || missingName === 'unresolved'

  return (
    <>
      {linkText == null ? (
        <span className="italic text-muted-foreground">未解決</span>
      ) : (
        <Link
          to="/charts/$instrumentId"
          params={{ instrumentId: symbol }}
          className="text-foreground hover:underline"
        >
          {linkText}
        </Link>
      )}
      {showSymbol && (
        <span className="ml-2 text-muted-foreground">{symbol}</span>
      )}
    </>
  )
}
