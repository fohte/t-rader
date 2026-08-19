import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
} from '@tanstack/react-router'

type StoryRoutePath =
  string | { path: string; component: () => React.ReactNode }

// Link や navigate が参照するパスを routeTree に登録するためだけのヘルパー。
// root route が Outlet を使わない story では子 route の component は描画されない。
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
