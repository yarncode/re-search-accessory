export const routes = [
  {
    path: '/',
    component: () => import('@/layouts/default.vue'),
    children: [
      {
        path: '',
        name: 'accessory-search',
        component: () => import('@/pages/index.vue'),
      },
      {
        path: 'bom',
        name: 'bom',
        component: () => import('@/pages/bom.vue'),
      },
      {
        path: 'viewer',
        name: 'viewer',
        component: () => import('@/pages/viewer.vue'),
      },
    ],
  },
  {
    path: '/:pathMatch(.*)*',
    redirect: '/',
  },
]
