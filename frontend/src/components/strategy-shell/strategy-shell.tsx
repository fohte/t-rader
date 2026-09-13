import type { ReactNode } from 'react'

import { FloatingChat } from '#components/strategy-shell/floating-chat'
import { Header } from '#components/strategy-shell/header'

const FRED_TERMS_OF_USE_URL =
  'https://fred.stlouisfed.org/docs/api/terms_of_use.html'

export function StrategyShell({ children }: { children: ReactNode }) {
  return (
    <div className="flex min-h-screen flex-col bg-background text-foreground">
      <Header />
      <main className="flex-1">
        <div className="mx-auto w-full max-w-7xl px-3 pb-20 pt-5 md:px-5 md:pt-6">
          {children}
        </div>
      </main>
      <footer className="border-t border-border px-3 py-3 pr-20 text-center font-mono text-xs text-muted-foreground-strong md:px-5 md:pr-24">
        <a
          href={FRED_TERMS_OF_USE_URL}
          target="_blank"
          rel="noreferrer"
          className="hover:text-foreground hover:underline"
        >
          This product uses the FRED® API but is not endorsed or certified by
          the Federal Reserve Bank of St. Louis.
        </a>
      </footer>
      <FloatingChat />
    </div>
  )
}
