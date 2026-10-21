import { ColorType } from 'lightweight-charts'

export interface ChartThemeColors {
  background: string
  textColor: string
  gridColor: string
  borderColor: string
}

// Lightweight Charts の color parser が OKLCH を扱えないため、sRGB に変換する。
function oklchToRgb(color: string): string | null {
  const match =
    /^oklch\(\s*(\d*\.?\d+)\s+(\d*\.?\d+)\s+(-?\d*\.?\d+)(?:deg)?\s*\)$/i.exec(
      color,
    )
  if (match == null) return null

  const [, lightnessValue, chromaValue, hueValue] = match
  if (lightnessValue == null || chromaValue == null || hueValue == null) {
    return null
  }

  const lightness = Number(lightnessValue)
  const chroma = Number(chromaValue)
  const hue = (Number(hueValue) * Math.PI) / 180
  if (![lightness, chroma, hue].every(Number.isFinite)) return null

  const a = chroma * Math.cos(hue)
  const b = chroma * Math.sin(hue)
  const lightnessL = (lightness + 0.3963377774 * a + 0.2158037573 * b) ** 3
  const lightnessM = (lightness - 0.1055613458 * a - 0.0638541728 * b) ** 3
  const lightnessS = (lightness - 0.0894841775 * a - 1.291485548 * b) ** 3

  const linearRgb = [
    4.0767416621 * lightnessL -
      3.3077115913 * lightnessM +
      0.2309699292 * lightnessS,
    -1.2684380046 * lightnessL +
      2.6097574011 * lightnessM -
      0.3413193965 * lightnessS,
    -0.0041960863 * lightnessL -
      0.7034186147 * lightnessM +
      1.707614701 * lightnessS,
  ]
  const srgb = linearRgb.map((channel) => {
    const clipped = Math.max(0, Math.min(1, channel))
    const encoded =
      clipped <= 0.0031308
        ? 12.92 * clipped
        : 1.055 * clipped ** (1 / 2.4) - 0.055
    return Math.round(encoded * 255)
  })

  return `rgb(${srgb.join(', ')})`
}

export function readChartColor(name: string): string {
  const color = getComputedStyle(document.documentElement)
    .getPropertyValue(name)
    .trim()
  return oklchToRgb(color) ?? color
}

export function chartColorWithOpacity(color: string, opacity: number): string {
  const match = /^rgb\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)$/i.exec(color)
  if (match == null) return color

  const [, red, green, blue] = match
  if (red == null || green == null || blue == null) return color

  return `rgba(${red}, ${green}, ${blue}, ${String(opacity)})`
}

export function getChartThemeColors(isDark: boolean): ChartThemeColors {
  return {
    background: isDark ? '#1a1a1a' : '#ffffff',
    textColor: isDark ? '#d1d5db' : '#374151',
    gridColor: isDark ? '#2d2d2d' : '#e5e7eb',
    borderColor: isDark ? '#3f3f46' : '#d1d5db',
  }
}

function resolveChartCssColor(
  variableName: `--${string}`,
  fallbackColor: string,
): string {
  if (typeof document === 'undefined') return fallbackColor

  return (
    getComputedStyle(document.documentElement)
      .getPropertyValue(variableName)
      .trim() || fallbackColor
  )
}

export function getAnnotationSelectionColor(isDark: boolean): string {
  return resolveChartCssColor(
    '--muted-foreground',
    getChartThemeColors(isDark).textColor,
  )
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
