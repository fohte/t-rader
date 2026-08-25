interface UnreadBadgeProps {
  count: number
  className?: string
}

export function UnreadBadge({ count, className = '' }: UnreadBadgeProps) {
  if (count <= 0) return null
  return (
    <span
      className={`inline-grid h-4 min-w-4 place-items-center bg-primary px-1 font-mono text-[10px] text-white ${className}`}
    >
      {count}
    </span>
  )
}
