import { AriaComponent } from 'echarts/components'
import type {
  ECElementEvent,
  EChartsCoreOption,
  EChartsType,
  Payload,
} from 'echarts/core'
import * as echarts from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'
import { Result } from 'neverthrow'
import { useCallback, useEffect, useRef, useState } from 'react'

import { observeChartResize, observeChartTheme } from '#lib/chart-theme'

echarts.use([AriaComponent, CanvasRenderer])

export interface EChartsPalette {
  background: string
  border: string
  foreground: string
  neutral: string
  up: string
  upDim: string
  down: string
  downDim: string
}

export type EChartsOptionFactory = (
  palette: EChartsPalette,
) => EChartsCoreOption

const fallbackPalette: EChartsPalette = {
  background: '#fafafa',
  border: '#e5e5e5',
  foreground: '#0a0a0a',
  neutral: '#1a1a1a',
  up: '#ef5350',
  upDim: '#5a2a2a',
  down: '#3f9fe0',
  downDim: '#234152',
}

function readPalette(element: HTMLElement): EChartsPalette {
  const styles = getComputedStyle(element)
  const read = (name: string, fallback: string) =>
    styles.getPropertyValue(name).trim() || fallback

  return {
    background: read('--color-background', fallbackPalette.background),
    border: read('--color-border', fallbackPalette.border),
    foreground: read('--color-foreground', fallbackPalette.foreground),
    neutral: read('--color-bg-tertiary', fallbackPalette.neutral),
    up: read('--color-up', fallbackPalette.up),
    upDim: read('--color-up-dim', fallbackPalette.upDim),
    down: read('--color-down', fallbackPalette.down),
    downDim: read('--color-down-dim', fallbackPalette.downDim),
  }
}

function boundaryError(error: unknown): EChartsBoundaryError {
  return new EChartsBoundaryError(error)
}

class EChartsBoundaryError extends Error {
  constructor(cause: unknown) {
    super('ECharts の描画に失敗しました。', { cause })
    this.name = new.target.name
  }
}

function applyOption(
  chart: EChartsType,
  createOption: EChartsOptionFactory,
  palette: EChartsPalette,
) {
  return Result.fromThrowable(() => {
    chart.setOption(createOption(palette), { notMerge: true })
  }, boundaryError)()
}

export function useEChartsChart({
  createOption,
  onClick,
  onRendered,
  onBeforeUpdate,
}: {
  createOption: EChartsOptionFactory
  onClick?: (event: ECElementEvent) => void
  onRendered?: () => void
  onBeforeUpdate?: () => void
}) {
  const containerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<EChartsType | null>(null)
  const paletteRef = useRef(fallbackPalette)
  const createOptionRef = useRef(createOption)
  const appliedFactoryRef = useRef(createOption)
  const handlersRef = useRef({ onClick, onRendered, onBeforeUpdate })
  const [palette, setPalette] = useState(fallbackPalette)
  const [error, setError] = useState<string | null>(null)

  createOptionRef.current = createOption
  handlersRef.current = { onClick, onRendered, onBeforeUpdate }

  const updateOption = useCallback(() => {
    const chart = chartRef.current
    if (chart == null) return

    handlersRef.current.onBeforeUpdate?.()
    const applied = applyOption(
      chart,
      createOptionRef.current,
      paletteRef.current,
    )
    applied.match(
      () => {
        setError(null)
      },
      (failure) => {
        setError(failure.message)
      },
    )
  }, [])

  const dispatchAction = useCallback((payload: Payload) => {
    const chart = chartRef.current
    if (chart == null) return

    const dispatched = Result.fromThrowable(() => {
      chart.dispatchAction(payload)
    }, boundaryError)()
    dispatched.match(
      () => {
        setError(null)
      },
      (failure) => {
        setError(failure.message)
      },
    )
  }, [])

  useEffect(() => {
    const container = containerRef.current
    if (container == null) return

    const initialized = Result.fromThrowable(
      () => echarts.init(container, undefined, { renderer: 'canvas' }),
      boundaryError,
    )()
    if (initialized.isErr()) {
      setError(initialized.error.message)
      return
    }

    const chart = initialized.value
    chartRef.current = chart
    const handleClick = (event: ECElementEvent) =>
      handlersRef.current.onClick?.(event)
    const handleFinished = () => handlersRef.current.onRendered?.()
    chart.on('click', handleClick)
    chart.on('finished', handleFinished)

    const updatePalette = (apply: boolean) => {
      const nextPalette = Result.fromThrowable(
        () => readPalette(container),
        boundaryError,
      )()
      nextPalette.match(
        (value) => {
          paletteRef.current = value
          setPalette(value)
          if (apply) {
            updateOption()
          } else {
            setError(null)
          }
        },
        (failure) => {
          setError(failure.message)
        },
      )
    }
    updatePalette(false)
    const disconnectResize = observeChartResize(container, () => {
      handlersRef.current.onBeforeUpdate?.()
      const resized = Result.fromThrowable(() => {
        chart.resize()
      }, boundaryError)()
      resized.match(
        () => {
          setError(null)
        },
        (failure) => {
          setError(failure.message)
        },
      )
    })
    const disconnectTheme = observeChartTheme(() => {
      updatePalette(true)
    })
    appliedFactoryRef.current = createOptionRef.current
    updateOption()

    return () => {
      disconnectResize()
      disconnectTheme()
      chart.off('click', handleClick)
      chart.off('finished', handleFinished)
      chartRef.current = null
      const disposed = Result.fromThrowable(() => {
        chart.dispose()
      }, boundaryError)()
      disposed.match(
        () => undefined,
        (failure) => {
          console.error('ECharts の破棄に失敗しました。', failure)
        },
      )
    }
  }, [updateOption])

  useEffect(() => {
    if (appliedFactoryRef.current === createOption) return
    appliedFactoryRef.current = createOption
    updateOption()
  }, [createOption, updateOption])

  return { containerRef, palette, error, updateOption, dispatchAction }
}
