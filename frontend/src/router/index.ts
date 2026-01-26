import { createRouter, createWebHistory, RouteRecordRaw } from 'vue-router';
import LiveRadar from '../views/LiveRadar.vue';
import FlightView from '../views/FlightView.vue';
import { getAuthService } from '@/services/authService';

const routes: Array<RouteRecordRaw> = [
  {
    path: '/',
    name: 'LiveRadar',
    component: LiveRadar,
  },
  {
    path: '/flightlog',
    name: 'flightlog',
    // route level code-splitting
    // this generates a separate chunk (flightlog.[hash].js) for this route
    // which is lazy-loaded when the route is visited.
    component: () => import(/* webpackChunkName: "about" */ '../views/FlightLog.vue'),
  },
  {
    path: '/flight/:flightId',
    name: 'flightview',
    component: FlightView,
  },
  {
    path: '/dashboard',
    name: 'dashboard',
    component: () => import('../views/AdminDashboard.vue'),
    meta: { requiresAdmin: true },
  },
  {
    path: '/dashboard/login',
    name: 'dashboardLogin',
    component: () => import('../views/AdminLogin.vue'),
  },
];

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes,
});

// Navigation guard for admin routes
router.beforeEach((to, _from, next) => {
  if (to.meta.requiresAdmin) {
    const authService = getAuthService();
    if (!authService.isAdmin()) {
      // Redirect to login if not admin
      next({ name: 'dashboardLogin' });
      return;
    }
  }
  next();
});

export default router;
