import { Link } from '@tanstack/react-router'

function isRecord(value: unknown): value is Record<string, unknown> {
  return value != null && typeof value === 'object' && !Array.isArray(value)
}

export function getScreenerIdFromFrontmatter(
  frontmatter: unknown,
): string | undefined {
  if (!isRecord(frontmatter)) return undefined

  const screenId = frontmatter.screen_id
  return typeof screenId === 'string' && screenId.trim() !== ''
    ? screenId
    : undefined
}

export function NoteScreenerLink({ screenId }: { screenId?: string }) {
  if (screenId == null) return null

  return (
    <Link
      to="/screener"
      search={{ screen: screenId }}
      className="inline-flex items-center gap-1 border border-border px-2.5 py-1 font-mono text-2xs text-muted-foreground-strong hover:border-primary hover:text-primary"
    >
      スクリーナーで開く <span aria-hidden="true">→</span>
    </Link>
  )
}
