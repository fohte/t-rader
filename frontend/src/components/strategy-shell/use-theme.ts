import { useCallback, useEffect, useRef, useState } from 'react'

const STORAGE_KEY = 't-rader-theme'

export type Theme = 'dark' | 'light'

function readInitial(): Theme {
  if (typeof window === 'undefined') return 'dark'
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY)
    if (stored === 'light' || stored === 'dark') return stored
  } catch {
    // localStorage が使えない環境 (Safari プライベートブラウジング等) では既定値で起動する
  }
  return 'dark'
}

export function useTheme() {
  const [theme, setTheme] = useState<Theme>(readInitial)
  // 初回マウントの effect で「ユーザー選択していない既定値」が localStorage に
  // 書き込まれると、OS 側プリファレンス変更を読めなくなるためスキップする
  const isFirst = useRef(true)

  useEffect(() => {
    const root = document.documentElement
    root.classList.toggle('dark', theme === 'dark')
    root.classList.toggle('light', theme === 'light')
    if (isFirst.current) {
      isFirst.current = false
      return
    }
    try {
      window.localStorage.setItem(STORAGE_KEY, theme)
    } catch {
      // localStorage 書き込み失敗時もテーマ適用自体は継続する
    }
  }, [theme])

  const toggle = useCallback(() => {
    setTheme((t) => (t === 'dark' ? 'light' : 'dark'))
  }, [])

  return { theme, toggle, setTheme }
}
