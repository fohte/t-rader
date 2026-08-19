import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
} from '@tanstack/react-router'

type StoryRoutePath =
  string | { path: string; component: () => React.ReactNode }

// paths の各エントリは Link/navigate が参照するパスを routeTree に登録する。
// component を渡した場合、root route が Outlet を描画する story でのみそれが実際に表示される。
export function createStoryRouter(
  component: () => React.ReactNode,
  options: { paths?: StoryRoutePath[]; initialPath?: string } = {},
) {
  const { paths = [], initialPath = '/' } = options
  const rootRoute = createRootRoute({ component })
  const childRoutes = paths.map((route) => {
    const path = typeof route === 'string' ? route : route.path
    const routeComponent =
      typeof route === 'string' ? () => null : route.component
    return createRoute({
      getParentRoute: () => rootRoute,
      path,
      component: routeComponent,
    })
  })

  return createRouter({
    routeTree: rootRoute.addChildren(childRoutes),
    history: createMemoryHistory({ initialEntries: [initialPath] }),
  })
}
