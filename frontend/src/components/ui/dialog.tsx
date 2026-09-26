'use client'

import { DialogContent as PackageDialogContent } from '@fohte/ui/dialog'
import * as React from 'react'

type DialogContentProps = React.ComponentProps<typeof PackageDialogContent>
type InitialFocusProp = DialogContentProps['initialFocus']
type InitialFocusResolver = Extract<
  InitialFocusProp,
  (...args: never[]) => unknown
>
type InteractionType = Parameters<InitialFocusResolver>[0]
type ResolvedFocus = ReturnType<InitialFocusResolver>

// initialFocus を Base UI の callback から返せる値に解決する。RefObject は
// callback の戻り値として扱えないため、current を返す。
function resolveInitialFocus(
  initialFocus: InitialFocusProp,
  openType: InteractionType,
  popupEl: HTMLDivElement | null,
): ResolvedFocus {
  if (typeof initialFocus === 'function') return initialFocus(openType)
  if (typeof initialFocus === 'boolean') return initialFocus
  if (initialFocus !== undefined) return initialFocus.current
  return openType === 'touch' ? popupEl : true
}

// native autoFocus は Base UI の初期フォーカス処理より先に発火するため、
// 先にフォーカス済みなら直接選択し、そうでなければ focusin を待つ。
function armSelectOnFocus(popupEl: HTMLDivElement | null) {
  const select = (el: EventTarget | Element | null) => {
    if (el instanceof HTMLInputElement) el.select()
  }
  if (popupEl?.contains(document.activeElement) === true) {
    select(document.activeElement)
  } else {
    popupEl?.addEventListener(
      'focusin',
      (event) => {
        select(event.target)
      },
      { once: true },
    )
  }
}

function DialogContent({ initialFocus, ref, ...props }: DialogContentProps) {
  const popupRef = React.useRef<HTMLDivElement>(null)

  return (
    <PackageDialogContent
      {...props}
      ref={(node) => {
        popupRef.current = node
        if (typeof ref === 'function') {
          return ref(node)
        }
        if (ref) {
          ref.current = node
        }
      }}
      initialFocus={(openType) => {
        const resolved = resolveInitialFocus(
          initialFocus,
          openType,
          popupRef.current,
        )
        // false/undefined はフォーカス移動しない指定なので、選択を待たない。
        if (resolved === false || resolved === undefined) {
          return resolved
        }
        armSelectOnFocus(popupRef.current)
        return resolved
      }}
    />
  )
}

export {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@fohte/ui/dialog'
export { DialogContent }
