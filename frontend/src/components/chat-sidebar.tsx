import { MessageSquare, X } from 'lucide-react'

import { Button } from '#components/ui/button'
import { cn } from '#lib/utils'

interface ChatSidebarProps {
  isOpen: boolean
  onClose: () => void
}

export function ChatSidebar({ isOpen, onClose }: ChatSidebarProps) {
  return (
    <aside
      data-testid="chat-sidebar"
      // aside 自体を box-content にして border 分を width の外側に出す。border-box のままだと
      // inner div の w-80 と border 込みの外枠幅が 1px ずれ、常に横方向にクリップされていた
      // transition は width のみに絞る。transition-all にすると border-l の出現に伴う
      // border-left-width の 0→1px 変化もアニメーションしてしまい、意図した snap 挙動が崩れる
      className={cn(
        'overflow-hidden bg-background transition-width duration-300 ease-in-out',
        isOpen ? 'box-content w-80 border-l' : 'w-0',
      )}
    >
      {isOpen && (
        <div className="flex h-full w-80 flex-col">
          <header className="flex h-14 shrink-0 items-center justify-between border-b px-4">
            <h2 className="font-semibold">AI チャット</h2>
            <Button
              variant="ghost"
              size="icon"
              onClick={onClose}
              aria-label="閉じる"
            >
              <X className="size-4" />
            </Button>
          </header>

          <div className="flex flex-1 flex-col items-center justify-center gap-3 p-4 text-muted-foreground">
            <MessageSquare className="size-10 opacity-40" />
            <p className="text-sm">AI チャット - 開発中</p>
          </div>
        </div>
      )}
    </aside>
  )
}
