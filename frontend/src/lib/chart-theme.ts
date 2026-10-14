import { ColorType } from 'lightweight-charts'

export interface ChartThemeColors {
  background: string
  textColor: string
  gridColor: string
  borderColor: string
}

export function getChartThemeColors(isDark: boolean): ChartThemeColors {
  return {
    background: isDark ? '#1a1a1a' : '#ffffff',
    textColor: isDark ? '#d1d5db' : '#374151',
    gridColor: isDark ? '#2d2d2d' : '#e5e7eb',
    borderColor: isDark ? '#3f3f46' : '#d1d5db',
  }
}

export function getChartThemeOptions(colors: ChartThemeColors) {
  return {
    layout: {
      background: { type: ColorType.Solid, color: colors.background },
      textColor: colors.textColor,
    },
    grid: {
      vertLines: { color: colors.gridColor },
      horzLines: { color: colors.gridColor },
    },
    timeScale: { borderColor: colors.borderColor },
    rightPriceScale: { borderColor: colors.borderColor },
  } as const
}

export function observeChartResize(
  container: HTMLElement,
  onResize: (width: number, height: number) => void,
): () => void {
  const observer = new ResizeObserver((entries) => {
    for (const entry of entries) {
      onResize(entry.contentRect.width, entry.contentRect.height)
    }
  })
  observer.observe(container)
  return () => {
    observer.disconnect()
  }
}

export function observeChartTheme(onThemeChange: () => void): () => void {
  const observer = new MutationObserver(onThemeChange)
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['class'],
  })
  return () => {
    observer.disconnect()
  }
}
