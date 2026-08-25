import { REF_KIND_JP, resolveRef } from '#lib/strategy-mock'

interface RefChipProps {
  // `stock:7203` のような prefix 付き token (markdown 中の [[...]] と同形式)
  token: string
  pill?: boolean
  showKind?: boolean
  onOpen?: (token: string) => void
}

export function RefChip({
  token,
  pill = false,
  showKind = true,
  onOpen,
}: RefChipProps) {
  const ref = resolveRef(token)
  const kindJP = REF_KIND_JP[ref.kind]

  const baseInner =
    'inline-flex items-baseline gap-1 font-mono text-em-88 leading-tight whitespace-nowrap text-foreground'
  const underline = 'border-b border-dotted border-muted-foreground pb-px'
  const pillCls = 'border border-border px-2 py-0.5 rounded-none'
  const wrapper = pill ? pillCls : underline
  const interactive = onOpen
    ? 'cursor-pointer hover:text-primary hover:border-primary'
    : ''
  const className = `${baseInner} ${wrapper} ${interactive}`.trim()

  const inner = (
    <>
      {showKind && (
        <span className="text-em-78 tracking-wide text-muted-foreground">
          {kindJP}
        </span>
      )}
      <span>{ref.name}</span>
      {ref.sub != null && ref.sub !== '' && (
        <span className="text-em-85 text-muted-foreground">{ref.sub}</span>
      )}
    </>
  )

  if (onOpen) {
    return (
      <button
        type="button"
        data-kind={ref.kind}
        title={`[[${token}]]`}
        onClick={(e) => {
          e.stopPropagation()
          onOpen(token)
        }}
        className={`${className} bg-transparent p-0`}
      >
        {inner}
      </button>
    )
  }

  return (
    <span data-kind={ref.kind} title={`[[${token}]]`} className={className}>
      {inner}
    </span>
  )
}
