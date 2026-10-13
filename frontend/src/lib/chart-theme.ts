import { ColorType, type IChartApi } from 'lightweight-charts'

export function getChartThemeOptions(isDark: boolean) {
  return {
    layout: {
      background: {
        type: ColorType.Solid,
        color: isDark ? '#1a1a1a' : '#ffffff',
      },
      textColor: isDark ? '#d1d5db' : '#374151',
    },
    grid: {
      vertLines: { color: isDark ? '#2d2d2d' : '#e5e7eb' },
      horzLines: { color: isDark ? '#2d2d2d' : '#e5e7eb' },
    },
    timeScale: { borderColor: isDark ? '#3f3f46' : '#d1d5db' },
    rightPriceScale: { borderColor: isDark ? '#3f3f46' : '#d1d5db' },
  }
}

export function observeChartTheme(chart: IChartApi): () => void {
  const observer = new MutationObserver(() => {
    chart.applyOptions(
      getChartThemeOptions(document.documentElement.classList.contains('dark')),
    )
  })
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['class'],
  })
  return () => {
    observer.disconnect()
  }
}
