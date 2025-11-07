import { createRouter, createWebHistory, RouteRecordRaw } from 'vue-router';
import LiveRadar from '../views/LiveRadar.vue';
import FlightView from '../views/FlightView.vue';

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
];

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes,
});

export default router;
