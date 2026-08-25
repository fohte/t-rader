'use client'

import { Dialog as DialogPrimitive } from '@base-ui/react/dialog'
import { XIcon } from 'lucide-react'
import * as React from 'react'

import { Button } from '#components/ui/button'
import { cn } from '#lib/utils'

function Dialog({ ...props }: DialogPrimitive.Root.Props) {
  return <DialogPrimitive.Root data-slot="dialog" {...props} />
}

function DialogTrigger({ ...props }: DialogPrimitive.Trigger.Props) {
  return <DialogPrimitive.Trigger data-slot="dialog-trigger" {...props} />
}

function DialogPortal({ ...props }: DialogPrimitive.Portal.Props) {
  return <DialogPrimitive.Portal data-slot="dialog-portal" {...props} />
}

function DialogClose({ ...props }: DialogPrimitive.Close.Props) {
  return <DialogPrimitive.Close data-slot="dialog-close" {...props} />
}

function DialogOverlay({
  className,
  ...props
}: DialogPrimitive.Backdrop.Props) {
  return (
    <DialogPrimitive.Backdrop
      data-slot="dialog-overlay"
      className={cn(
        'data-open:animate-in data-closed:animate-out data-closed:fade-out-0 data-open:fade-in-0 fixed inset-0 z-50 bg-black/50',
        className,
      )}
      {...props}
    />
  )
}

type InitialFocusProp = DialogPrimitive.Popup.Props['initialFocus']
type InitialFocusResolver = Extract<
  InitialFocusProp,
  (...args: never[]) => unknown
>
type InteractionType = Parameters<InitialFocusResolver>[0]
type ResolvedFocus = ReturnType<InitialFocusResolver>

// caller の initialFocus 指定 (未指定/boolean/RefObject/関数) を Base UI に返せる
// 値に解決する。関数版の戻り値は boolean | HTMLElement | null | void のみで
// RefObject は返せないため、RefObject の場合は .current を返す。未指定時の
// touch 時のデフォルト挙動 (仮想キーボード抑止のため popup 自体にフォーカス) は
// Base UI 内部の defaultInitialFocus と同じロジックで再現する
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

// dialog 表示時の初期フォーカス先で input のテキストを全選択する。native
// autoFocus は Base UI の initialFocus (FloatingFocusManager の layout effect)
// より先に発火するため、その場合は activeElement を直接 select() し、そうで
// なければ Base UI がフォーカスを移すのを focusin で待つ
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

function DialogContent({
  className,
  children,
  showCloseButton = true,
  initialFocus,
  ref,
  ...props
}: DialogPrimitive.Popup.Props & {
  showCloseButton?: boolean
}) {
  const popupRef = React.useRef<HTMLDivElement>(null)

  return (
    <DialogPortal data-slot="dialog-portal">
      <DialogOverlay />
      <DialogPrimitive.Popup
        ref={(node) => {
          popupRef.current = node
          if (typeof ref === 'function') {
            return ref(node)
          }
          if (ref) {
            ref.current = node
          }
        }}
        data-slot="dialog-content"
        initialFocus={(openType) => {
          const resolved = resolveInitialFocus(
            initialFocus,
            openType,
            popupRef.current,
          )
          // resolved === undefined (関数が値を返さない) も Base UI 側では
          // false と同様「フォーカス移動なし」として扱われるため、ここでも
          // select 用のリスナーを仕込まずに素通しする
          if (resolved === false || resolved === undefined) {
            return resolved
          }
          armSelectOnFocus(popupRef.current)
          return resolved
        }}
        className={cn(
          'bg-background data-open:animate-in data-closed:animate-out data-closed:fade-out-0 data-open:fade-in-0 data-closed:zoom-out-95 data-open:zoom-in-95 fixed top-[50%] left-[50%] z-50 grid w-full max-w-[calc(100%-2rem)] translate-x-[-50%] translate-y-[-50%] gap-4 rounded-lg border p-6 shadow-lg duration-200 outline-none sm:max-w-lg',
          className,
        )}
        {...props}
      >
        {children}
        {showCloseButton && (
          <DialogPrimitive.Close
            data-slot="dialog-close"
            className="ring-offset-background focus:ring-ring data-open:bg-accent data-open:text-muted-foreground absolute top-4 right-4 rounded-xs opacity-70 transition-opacity hover:opacity-100 focus:ring-2 focus:ring-offset-2 focus:outline-hidden disabled:pointer-events-none [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4"
          >
            <XIcon />
            <span className="sr-only">Close</span>
          </DialogPrimitive.Close>
        )}
      </DialogPrimitive.Popup>
    </DialogPortal>
  )
}

function DialogHeader({ className, ...props }: React.ComponentProps<'div'>) {
  return (
    <div
      data-slot="dialog-header"
      className={cn('flex flex-col gap-2 text-center sm:text-left', className)}
      {...props}
    />
  )
}

function DialogFooter({
  className,
  showCloseButton = false,
  children,
  ...props
}: React.ComponentProps<'div'> & {
  showCloseButton?: boolean
}) {
  return (
    <div
      data-slot="dialog-footer"
      className={cn(
        'flex flex-col-reverse gap-2 sm:flex-row sm:justify-end',
        className,
      )}
      {...props}
    >
      {children}
      {showCloseButton && (
        <DialogPrimitive.Close
          render={<Button variant="outline">Close</Button>}
        />
      )}
    </div>
  )
}

function DialogTitle({ className, ...props }: DialogPrimitive.Title.Props) {
  return (
    <DialogPrimitive.Title
      data-slot="dialog-title"
      className={cn('text-lg leading-none font-semibold', className)}
      {...props}
    />
  )
}

function DialogDescription({
  className,
  ...props
}: DialogPrimitive.Description.Props) {
  return (
    <DialogPrimitive.Description
      data-slot="dialog-description"
      className={cn('text-muted-foreground text-sm', className)}
      {...props}
    />
  )
}

export {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogOverlay,
  DialogPortal,
  DialogTitle,
  DialogTrigger,
}
