import {
  Children,
  cloneElement,
  Fragment,
  isValidElement,
  type ReactNode,
} from 'react'

const TEXT_BOUNDARY = '\u0000'

interface TextHighlightOptions {
  textOffset?: number
  refChipTexts?: Record<string, string>
}

export interface TextHighlightResult {
  children: ReactNode
  match: 'missing' | 'found'
}

function isTextBoundary(
  type: unknown,
  props: { children?: ReactNode },
): boolean {
  return type === 'br' || (type !== Fragment && props.children == null)
}

function isReferenceToken(token: string | undefined): token is string {
  return (
    token?.startsWith('stock:') === true ||
    token?.startsWith('indicator:') === true ||
    token?.startsWith('group:') === true
  )
}

function textContent(
  node: ReactNode,
  refChipTexts: Record<string, string>,
): string {
  return Children.toArray(node)
    .map((child) => {
      if (typeof child === 'string' || typeof child === 'number') {
        return String(child)
      }
      if (
        !isValidElement<{
          children?: ReactNode
          commentTextHighlight?: string
          token?: string
        }>(child)
      )
        return ''

      const props = child.props
      if (isReferenceToken(props.token)) {
        return refChipTexts[props.token] ?? TEXT_BOUNDARY
      }
      if (isTextBoundary(child.type, props)) {
        return TEXT_BOUNDARY
      }
      return textContent(props.children, refChipTexts)
    })
    .join('')
}

function withHighlight(
  node: ReactNode,
  start: number,
  end: number,
  offset: { value: number },
  refChipTexts: Record<string, string>,
): ReactNode {
  const highlightedChildren = Children.map(node, (child) => {
    if (typeof child === 'string' || typeof child === 'number') {
      const text = String(child)
      const childStart = offset.value
      const childEnd = childStart + text.length
      offset.value = childEnd

      if (childStart >= end || childEnd <= start) return child

      const highlightStart = Math.max(0, start - childStart)
      const highlightEnd = Math.min(text.length, end - childStart)
      return (
        <Fragment>
          {text.slice(0, highlightStart)}
          <mark
            data-comment-text-highlight="true"
            className="bg-muted-foreground/20 text-inherit"
          >
            {text.slice(highlightStart, highlightEnd)}
          </mark>
          {text.slice(highlightEnd)}
        </Fragment>
      )
    }
    if (
      !isValidElement<{
        children?: ReactNode
        commentTextHighlight?: string
        token?: string
      }>(child)
    )
      return child

    const props = child.props
    if (isReferenceToken(props.token)) {
      const chipText = refChipTexts[props.token]
      if (chipText == null) {
        offset.value += 1
        return child
      }
      const childStart = offset.value
      const childEnd = childStart + chipText.length
      offset.value = childEnd
      if (childStart >= end || childEnd <= start) return child
      const highlightStart = Math.max(0, start - childStart)
      const highlightEnd = Math.min(chipText.length, end - childStart)
      return cloneElement(child, {
        commentTextHighlight: chipText.slice(highlightStart, highlightEnd),
      })
    }
    if (isTextBoundary(child.type, props)) {
      offset.value += 1
      return child
    }

    return cloneElement(
      child,
      undefined,
      withHighlight(props.children, start, end, offset, refChipTexts),
    )
  })
  return isValidElement(node) && Array.isArray(highlightedChildren)
    ? (highlightedChildren[0] ?? node)
    : highlightedChildren
}

export function highlightFirstTextMatch(
  children: ReactNode,
  text: string,
  options: TextHighlightOptions = {},
): TextHighlightResult {
  const refChipTexts = options.refChipTexts ?? {}
  const content = textContent(children, refChipTexts)
  const preferredOffset = options.textOffset
  const start =
    preferredOffset != null &&
    content.slice(preferredOffset, preferredOffset + text.length) === text
      ? preferredOffset
      : content.indexOf(text)
  if (start === -1) return { children, match: 'missing' }

  const end = start + text.length
  return {
    children: withHighlight(children, start, end, { value: 0 }, refChipTexts),
    match: 'found',
  }
}
