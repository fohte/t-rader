import { Button } from '@fohte/ui/button'
import { type ReactNode, useEffect } from 'react'

import type { ResolvedRef } from '#hooks/use-resolve-ref'
import { useResolveRef } from '#hooks/use-resolve-ref'
import { parseGroupRefId } from '#lib/ref-id'
import { REF_KIND_JP } from '#lib/strategy-mock'
import { cn } from '#lib/utils'

interface RefChipProps {
  // `stock:demo-code` のような prefix 付き token (markdown 中の [[...]] と同形式)
  token: string
  pill?: boolean
  showKind?: boolean
  // 画面遷移先がある参照に限って呼び出す。
  onOpenRef?: (token: string, resolved: ResolvedRef) => void
  onTextContentChange?: (token: string, text: string) => void
  commentTextHighlight?: string
}

export function RefChip({
  token,
  pill = false,
  showKind = true,
  onOpenRef,
  onTextContentChange,
  commentTextHighlight,
}: RefChipProps) {
  const ref = useResolveRef(token)
  const kindJP = REF_KIND_JP[ref.kind]
  const resolved = ref.name != null
  const displayName = ref.name ?? ref.id
  const sub =
    resolved && ref.kind === 'stock' && ref.id !== ref.name ? ref.id : undefined
  const chipText = `${showKind ? kindJP : ''}${displayName}${sub ?? ''}`
  useEffect(() => {
    onTextContentChange?.(token, chipText)
  }, [chipText, onTextContentChange, token])
  const highlightStart =
    commentTextHighlight == null ? -1 : chipText.indexOf(commentTextHighlight)
  const highlightEnd =
    highlightStart === -1
      ? -1
      : highlightStart + (commentTextHighlight?.length ?? 0)
  const canOpenRef =
    onOpenRef != null &&
    (ref.kind === 'stock' ||
      (ref.id !== '' &&
        (ref.kind !== 'group' || parseGroupRefId(ref.id) != null)))

  const baseInner =
    'inline-flex items-baseline gap-1 font-mono text-em-88 leading-tight whitespace-nowrap'
  const underline = 'border-b border-dotted border-muted-foreground pb-px'
  const pillCls = 'rounded-none border border-border px-2 py-0.5'
  const wrapper = pill ? pillCls : underline
  const className = cn(baseInner, wrapper, 'text-foreground')
  const title = resolved ? `[[${token}]]` : `[[${token}]] (未解決)`

  const highlightedText = (text: string, segmentStart: number): ReactNode => {
    const start = Math.max(highlightStart, segmentStart)
    const end = Math.min(highlightEnd, segmentStart + text.length)
    if (start >= end) return text
    const localStart = start - segmentStart
    const localEnd = end - segmentStart
    return (
      <>
        {text.slice(0, localStart)}
        <mark
          data-comment-text-highlight="true"
          className="bg-muted text-inherit"
        >
          {text.slice(localStart, localEnd)}
        </mark>
        {text.slice(localEnd)}
      </>
    )
  }
  const nameStart = showKind ? kindJP.length : 0
  const subStart = nameStart + displayName.length

  const inner = (
    <>
      {showKind && (
        <span className="text-em-78 tracking-wide text-muted-foreground">
          {highlightedText(kindJP, 0)}
        </span>
      )}
      <span
        className={cn(
          resolved
            ? 'text-foreground group-hover/button:text-destructive'
            : 'italic text-muted-foreground group-hover/button:text-destructive',
        )}
      >
        {highlightedText(displayName, nameStart)}
      </span>
      {sub != null && sub !== '' && (
        <span className="text-em-85 text-muted-foreground">
          {highlightedText(sub, subStart)}
        </span>
      )}
    </>
  )

  if (canOpenRef) {
    return (
      <Button
        variant="plain"
        type="button"
        data-kind={ref.kind}
        title={title}
        onClick={(e) => {
          e.stopPropagation()
          onOpenRef(token, ref)
        }}
      >
        <span className={cn(baseInner, wrapper)}>{inner}</span>
      </Button>
    )
  }

  return (
    <span data-kind={ref.kind} title={title} className={className}>
      {inner}
    </span>
  )
}
