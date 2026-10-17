import {
  Children,
  cloneElement,
  Fragment,
  isValidElement,
  type ReactNode,
} from 'react'

const TEXT_BOUNDARY = '\u0000'

export interface TextHighlightResult {
  children: ReactNode
  match: 'missing' | 'partial' | 'full'
}

function textContent(node: ReactNode): string {
  return Children.toArray(node)
    .map((child) => {
      if (typeof child === 'string' || typeof child === 'number') {
        return String(child)
      }
      if (!isValidElement<{ children?: ReactNode }>(child)) return ''

      const props = child.props
      if (
        child.type === 'br' ||
        (child.type !== Fragment && props.children == null)
      ) {
        return TEXT_BOUNDARY
      }
      return textContent(props.children)
    })
    .join('')
}

function withHighlight(
  node: ReactNode,
  start: number,
  end: number,
  offset: { value: number },
): ReactNode {
  return Children.map(node, (child) => {
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
            className="bg-primary/20 text-inherit"
          >
            {text.slice(highlightStart, highlightEnd)}
          </mark>
          {text.slice(highlightEnd)}
        </Fragment>
      )
    }
    if (!isValidElement<{ children?: ReactNode }>(child)) return child

    const props = child.props
    if (
      child.type === 'br' ||
      (child.type !== Fragment && props.children == null)
    ) {
      offset.value += 1
      return child
    }

    return cloneElement(
      child,
      undefined,
      withHighlight(props.children, start, end, offset),
    )
  })
}

export function highlightFirstTextMatch(
  children: ReactNode,
  text: string,
): TextHighlightResult {
  const content = textContent(children)
  const start = content.indexOf(text)
  if (start === -1) return { children, match: 'missing' }

  const end = start + text.length
  const match = start === 0 && end === content.length ? 'full' : 'partial'
  return {
    children:
      match === 'partial'
        ? withHighlight(children, start, end, { value: 0 })
        : children,
    match,
  }
}
