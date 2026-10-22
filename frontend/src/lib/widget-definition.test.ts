import { describe, expect, it } from 'vitest'
import { parseDocument } from 'yaml'

import { parseWidgetDefinition } from '#lib/widget-definition'

const VALID_WIDGET_DEFINITION = [
  'queries:',
  '  daily:',
  '    metric: change',
  '    period: 1d',
  '    by: group',
  '    axis: demo-axis',
  '    agg: mean',
  'formulas:',
  '  doubled: daily * 2',
  'display:',
  '  daily:',
  '    label: Daily movement',
  '    format: percent',
  '  doubled:',
  '    label: Derived value',
  '    format: number',
  'viz:',
  '  type: toplist',
  '  value: doubled',
  '  order: desc',
  '  limit: 5',
  '  bar_scale:',
  '    min: -1',
  '    max: 1',
  'time:',
  '  as_of: 2099-01-05',
].join('\n')

const VALID_RESULT = {
  ok: true,
  request: {
    queries: {
      daily: {
        metric: 'change',
        period: '1d',
        by: 'group',
        axis: 'demo-axis',
        agg: 'mean',
      },
    },
    formulas: { doubled: 'daily * 2' },
    display: {
      daily: { label: 'Daily movement', format: 'percent' },
      doubled: { label: 'Derived value', format: 'number' },
    },
    viz: {
      type: 'toplist',
      value: 'doubled',
      order: 'desc',
      limit: 5,
      bar_scale: { min: -1, max: 1 },
    },
    time: { as_of: '2099-01-05' },
  },
  viz: {
    type: 'toplist',
    value: 'doubled',
    order: 'desc',
    limit: 5,
    bar_scale: { min: -1, max: 1 },
  },
  asOf: '2099-01-05',
}
const TABLE_WIDGET_DEFINITION = VALID_WIDGET_DEFINITION.replace(
  'type: toplist\n  value: doubled\n  order: desc\n  limit: 5\n  bar_scale:\n    min: -1\n    max: 1',
  'type: table\n  columns:\n    - doubled\n    - daily',
)
const LINE_WIDGET_DEFINITION = VALID_WIDGET_DEFINITION.replace(
  'type: toplist\n  value: doubled\n  order: desc\n  limit: 5\n  bar_scale:\n    min: -1\n    max: 1',
  'type: line\n  series:\n    - doubled',
).replace(
  'time:\n  as_of: 2099-01-05',
  'time:\n  as_of: 2099-01-05\n  range: 1y',
)

describe('parseWidgetDefinition', () => {
  it('parses YAML into a widget query request', () => {
    expect(parseWidgetDefinition(VALID_WIDGET_DEFINITION)).toEqual(VALID_RESULT)
  })

  it('preserves the configured table column order', () => {
    expect(parseWidgetDefinition(TABLE_WIDGET_DEFINITION)).toEqual({
      ok: true,
      request: {
        ...VALID_RESULT.request,
        viz: { type: 'table', columns: ['doubled', 'daily'] },
      },
      viz: { type: 'table', columns: ['doubled', 'daily'] },
      asOf: '2099-01-05',
    })
  })

  it('accepts a line with a relative date range', () => {
    expect(parseWidgetDefinition(LINE_WIDGET_DEFINITION)).toEqual({
      ok: true,
      request: {
        ...VALID_RESULT.request,
        viz: { type: 'line', series: ['doubled'] },
        time: { as_of: '2099-01-05', range: '1y' },
      },
      viz: { type: 'line', series: ['doubled'] },
      asOf: '2099-01-05',
    })
  })

  it.each([
    { name: '3m', range: '3m' },
    { name: '1y', range: '1y' },
    {
      name: 'an inclusive date range',
      range: '2099-01-01..2099-01-05',
    },
  ])('accepts $name for a line', ({ range }) => {
    expect(
      parseWidgetDefinition(
        LINE_WIDGET_DEFINITION.replace('range: 1y', `range: ${range}`),
      ),
    ).toEqual({
      ok: true,
      request: {
        ...VALID_RESULT.request,
        viz: { type: 'line', series: ['doubled'] },
        time: { as_of: '2099-01-05', range },
      },
      viz: { type: 'line', series: ['doubled'] },
      asOf: '2099-01-05',
    })
  })

  it.each([
    {
      name: 'time',
      source: VALID_WIDGET_DEFINITION.replace('time:\n  as_of: 2099-01-05', ''),
      request: {
        queries: VALID_RESULT.request.queries,
        formulas: VALID_RESULT.request.formulas,
        display: VALID_RESULT.request.display,
        viz: VALID_RESULT.request.viz,
      },
    },
    {
      name: 'time.as_of',
      source: VALID_WIDGET_DEFINITION.replace(
        'time:\n  as_of: 2099-01-05',
        'time: {}',
      ),
      request: { ...VALID_RESULT.request, time: {} },
    },
  ])('allows omitted $name', ({ source, request }) => {
    expect(parseWidgetDefinition(source)).toEqual({
      ok: true,
      request,
      viz: VALID_RESULT.viz,
      asOf: undefined,
    })
  })

  it('passes semantic validation values through for the API to validate', () => {
    const source = VALID_WIDGET_DEFINITION.replace(
      'metric: change',
      'metric: future-metric',
    )
      .replace('period: 1d', 'period: future-period')
      .replace('by: group', 'by: future-dimension')
      .replace('agg: mean', 'agg: future-aggregation')
      .replace('type: toplist', 'type: future-viz')
      .replace('order: desc', 'order: sideways')
      .replace('limit: 5', 'limit: -1')
      .replace('as_of: 2099-01-05', 'as_of: not-a-date')

    expect(parseWidgetDefinition(source)).toEqual({
      ok: true,
      request: {
        queries: {
          daily: {
            metric: 'future-metric',
            period: 'future-period',
            by: 'future-dimension',
            axis: 'demo-axis',
            agg: 'future-aggregation',
          },
        },
        formulas: { doubled: 'daily * 2' },
        display: VALID_RESULT.request.display,
        viz: {
          type: 'future-viz',
          value: 'doubled',
          order: 'sideways',
          limit: -1,
          bar_scale: { min: -1, max: 1 },
        },
        time: { as_of: 'not-a-date' },
      },
      viz: {
        type: 'future-viz',
        value: 'doubled',
        order: 'sideways',
        limit: -1,
        bar_scale: { min: -1, max: 1 },
      },
      asOf: 'not-a-date',
    })
  })

  it('reports unsupported widget fields with their path', () => {
    expect(
      parseWidgetDefinition(
        VALID_WIDGET_DEFINITION.replace('formulas:', 'formula:'),
      ),
    ).toEqual({
      ok: false,
      path: 'formula',
      message: '未対応の項目です。',
    })
  })

  it.each([
    {
      name: 'a query with a non-string metric',
      source: VALID_WIDGET_DEFINITION.replace('metric: change', 'metric: 1'),
      path: undefined,
      message: 'queries.daily の形式が正しくありません。',
    },
    {
      name: 'a universe with both group and stocks',
      source: VALID_WIDGET_DEFINITION.replace(
        '    agg: mean',
        '    agg: mean\n    universe:\n      group: demo-group\n      stocks:\n        - demo-code',
      ),
      path: undefined,
      message: 'queries.daily の形式が正しくありません。',
    },
    {
      name: 'a formula with a non-string value',
      source: VALID_WIDGET_DEFINITION.replace(
        'doubled: daily * 2',
        'doubled: 2',
      ),
      path: undefined,
      message: 'formulas は文字列のマップにしてください。',
    },
    {
      name: 'a widget with a non-string time.as_of',
      source: VALID_WIDGET_DEFINITION.replace(
        '  as_of: 2099-01-05',
        '  as_of: 1',
      ),
      path: undefined,
      message: 'time.as_of は文字列で指定してください。',
    },
    {
      name: 'a widget with time.range',
      source: VALID_WIDGET_DEFINITION.replace(
        '  as_of: 2099-01-05',
        '  as_of: 2099-01-05\n  range: 1d',
      ),
      path: 'time.range',
      message: 'time.range は line 以外では使えません。',
    },
    {
      name: 'a widget with a null time.range',
      source: VALID_WIDGET_DEFINITION.replace(
        '  as_of: 2099-01-05',
        '  as_of: 2099-01-05\n  range: null',
      ),
      path: 'time.range',
      message: 'time.range は line 以外では使えません。',
    },
    {
      name: 'a table with time.range',
      source: TABLE_WIDGET_DEFINITION.replace(
        '  as_of: 2099-01-05',
        '  as_of: 2099-01-05\n  range: 1d',
      ),
      path: 'time.range',
      message: 'time.range は line 以外では使えません。',
    },
    {
      name: 'a line without time.range',
      source: LINE_WIDGET_DEFINITION.replace('\n  range: 1y', ''),
      path: 'time.range',
      message:
        'time.range は line で 3m、1y、または YYYY-MM-DD..YYYY-MM-DD を指定してください。',
    },
    {
      name: 'a line with an unsupported relative range',
      source: LINE_WIDGET_DEFINITION.replace('range: 1y', 'range: 6m'),
      path: 'time.range',
      message:
        'time.range は line で 3m、1y、または YYYY-MM-DD..YYYY-MM-DD を指定してください。',
    },
    {
      name: 'a line with an impossible calendar date',
      source: LINE_WIDGET_DEFINITION.replace(
        'range: 1y',
        'range: 2099-02-30..2099-03-01',
      ),
      path: 'time.range',
      message:
        'time.range は line で 3m、1y、または YYYY-MM-DD..YYYY-MM-DD を指定してください。',
    },
    {
      name: 'a line with reversed date bounds',
      source: LINE_WIDGET_DEFINITION.replace(
        'range: 1y',
        'range: 2099-01-06..2099-01-05',
      ),
      path: 'time.range',
      message:
        'time.range は line で 3m、1y、または YYYY-MM-DD..YYYY-MM-DD を指定してください。',
    },
    {
      name: 'a line that ends after its as_of date',
      source: LINE_WIDGET_DEFINITION.replace(
        'range: 1y',
        'range: 2099-01-05..2099-01-06',
      ),
      path: 'time.range',
      message: 'time.range の終了日は time.as_of 以下にしてください。',
    },
  ])('rejects $name', ({ source, path, message }) => {
    expect(parseWidgetDefinition(source)).toEqual({
      ok: false,
      ...(path == null ? {} : { path }),
      message,
    })
  })

  it('reports YAML syntax errors', () => {
    const source = 'queries: ['
    const parserMessage =
      parseDocument(source, { uniqueKeys: true }).errors[0]?.message ??
      '解析エラーがありません。'

    expect(parseWidgetDefinition(source)).toEqual({
      ok: false,
      message: `YAML を解析できません: ${parserMessage}`,
    })
  })
})
