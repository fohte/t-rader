import { createFileRoute } from '@tanstack/react-router'

import { SettingsIndexItems } from '#components/settings/settings-index-items'

export const Route = createFileRoute('/settings/')({
  component: SettingsIndexPage,
})

function SettingsIndexPage() {
  return (
    <div className="space-y-5">
      <header>
        <h1 className="mb-1 text-2xl font-bold leading-tight tracking-tight">
          設定
        </h1>
        <p className="text-sm text-muted-foreground-strong">
          グローバル設定の一覧。
        </p>
      </header>
      <SettingsIndexItems />
    </div>
  )
}
