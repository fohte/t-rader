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
    // eslint-disable-next-line no-restricted-syntax -- text-[0.88em] は親要素のフォントサイズ相対値で @theme に token 化されていない
    'inline-flex items-baseline gap-1 font-mono text-[0.88em] leading-tight whitespace-nowrap text-text-primary'
  const underline = 'border-b border-dotted border-text-tertiary pb-px'
  const pillCls = 'border border-border-strategy px-2 py-0.5 rounded-none'
  const wrapper = pill ? pillCls : underline
  const interactive = onOpen
    ? 'cursor-pointer hover:text-[color:var(--color-accent-strategy)] hover:border-[color:var(--color-accent-strategy)]'
    : ''
  const className = `${baseInner} ${wrapper} ${interactive}`.trim()

  const inner = (
    <>
      {showKind && (
        <span className="text-[0.78em] tracking-wide text-[color:var(--color-text-tertiary)]">
          {kindJP}
        </span>
      )}
      <span>{ref.name}</span>
      {ref.sub != null && ref.sub !== '' && (
        <span className="text-[0.85em] text-[color:var(--color-text-tertiary)]">
          {ref.sub}
        </span>
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
