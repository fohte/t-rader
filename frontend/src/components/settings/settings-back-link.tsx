import { Link } from '@tanstack/react-router'

export function SettingsBackLink() {
  return (
    <Link
      to="/settings"
      className="font-mono text-xs text-muted-foreground hover:text-foreground"
    >
      &lt; 設定に戻る
    </Link>
  )
}
