import type { LucideIcon } from 'lucide-react'
import { Construction } from 'lucide-react'

interface PlaceholderPageProps {
  title: string
  description: string
  icon: LucideIcon
}

export function PlaceholderPage({
  title,
  description,
  icon: Icon,
}: PlaceholderPageProps) {
  return (
    <div className="flex h-full items-center justify-center">
      <div className="max-w-md text-center">
        <Icon className="mx-auto size-12 text-muted-foreground" />
        <h1 className="mt-4 text-2xl font-bold">{title}</h1>
        <p className="mt-2 text-muted-foreground">{description}</p>
        <div className="mt-6 flex items-center justify-center gap-2 text-sm text-muted-foreground">
          <Construction className="size-4" />
          <span>この機能は開発中です</span>
        </div>
      </div>
    </div>
  )
}
