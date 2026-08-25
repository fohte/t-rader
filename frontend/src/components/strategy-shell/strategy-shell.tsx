import type { ReactNode } from 'react'

import { FloatingChat } from '#components/strategy-shell/floating-chat'
import { Header } from '#components/strategy-shell/header'

export function StrategyShell({ children }: { children: ReactNode }) {
  return (
    <div className="flex min-h-screen flex-col bg-background text-foreground">
      <Header />
      <main className="flex-1">
        <div className="mx-auto w-full max-w-[1280px] px-3 pb-20 pt-5 md:px-5 md:pt-6">
          {children}
        </div>
      </main>
      <FloatingChat />
    </div>
  )
}
