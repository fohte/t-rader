import { MessageSquare } from 'lucide-react'
import { type ReactNode, useState } from 'react'

import { AppSidebar } from '@/components/app-sidebar'
import { ChatSidebar } from '@/components/chat-sidebar'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from '@/components/ui/sidebar'
import { TooltipProvider } from '@/components/ui/tooltip'

export function AppShell({ children }: { children: ReactNode }) {
  const [isChatOpen, setIsChatOpen] = useState(false)

  return (
    <TooltipProvider>
      <SidebarProvider>
        <AppSidebar />
        <SidebarInset>
          <header className="flex h-14 shrink-0 items-center gap-2 border-b px-4">
            <SidebarTrigger className="-ml-1" />
            <Separator orientation="vertical" className="mr-2 !h-4" />
            <h1 className="text-lg font-semibold">T-Rader</h1>
            <div className="ml-auto">
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setIsChatOpen((prev) => !prev)}
                aria-label="AI チャット"
              >
                <MessageSquare className="size-4" />
              </Button>
            </div>
          </header>
          <div className="flex-1 p-4">{children}</div>
        </SidebarInset>
        <ChatSidebar isOpen={isChatOpen} onClose={() => setIsChatOpen(false)} />
      </SidebarProvider>
    </TooltipProvider>
  )
}
