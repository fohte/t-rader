import type { ResolvedRef } from '#hooks/use-resolve-ref'
import { useResolveRef } from '#hooks/use-resolve-ref'
import { parseGroupRefId } from '#lib/ref-id'
import { REF_KIND_JP } from '#lib/strategy-mock'

interface RefChipProps {
  // `stock:demo-code` のような prefix 付き token (markdown 中の [[...]] と同形式)
  token: string
  pill?: boolean
  showKind?: boolean
  // 画面遷移先がある参照に限って呼び出す。
  onOpenRef?: (token: string, resolved: ResolvedRef) => void
}

export function RefChip({
  token,
  pill = false,
  showKind = true,
  onOpenRef,
}: RefChipProps) {
  const ref = useResolveRef(token)
  const kindJP = REF_KIND_JP[ref.kind]
  const resolved = ref.name != null
  const displayName = ref.name ?? ref.id
  const sub =
    resolved && ref.kind === 'stock' && ref.id !== ref.name ? ref.id : undefined
  const canOpenRef =
    onOpenRef != null &&
    (ref.kind === 'stock' ||
      (ref.id !== '' &&
        (ref.kind !== 'group' || parseGroupRefId(ref.id) != null)))

  const baseInner =
    'inline-flex items-baseline gap-1 font-mono text-em-88 leading-tight whitespace-nowrap text-foreground'
  const underline = 'border-b border-dotted border-muted-foreground pb-px'
  const pillCls = 'border border-border px-2 py-0.5 rounded-none'
  const wrapper = pill ? pillCls : underline
  const interactive = canOpenRef
    ? 'cursor-pointer hover:text-primary hover:border-primary'
    : ''
  const className = `${baseInner} ${wrapper} ${interactive}`.trim()
  const title = resolved ? `[[${token}]]` : `[[${token}]] (未解決)`

  const inner = (
    <>
      {showKind && (
        <span className="text-em-78 tracking-wide text-muted-foreground">
          {kindJP}
        </span>
      )}
      <span className={resolved ? undefined : 'italic text-muted-foreground'}>
        {displayName}
      </span>
      {sub != null && sub !== '' && (
        <span className="text-em-85 text-muted-foreground">{sub}</span>
      )}
    </>
  )

  if (canOpenRef) {
    return (
      <button
        type="button"
        data-kind={ref.kind}
        title={title}
        onClick={(e) => {
          e.stopPropagation()
          onOpenRef(token, ref)
        }}
        className={`${className} bg-transparent p-0`}
      >
        {inner}
      </button>
    )
  }

  return (
    <span data-kind={ref.kind} title={title} className={className}>
      {inner}
    </span>
  )
}
