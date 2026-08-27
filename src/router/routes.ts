import type { RouteRecordRaw } from 'vue-router'

export const routes: RouteRecordRaw[] = [
  {
    path: '/',
    redirect: '/bar',
  },
  {
    path: '/bar',
    name: 'bar',
    component: () => import('@/pages/bar/index.vue'),
  },
  {
    path: '/settings',
    name: 'settings',
    component: () => import('@/pages/settings/index.vue'),
  },
  {
    path: '/volume',
    name: 'volume',
    component: () => import('@/pages/volume/index.vue'),
  },
]
