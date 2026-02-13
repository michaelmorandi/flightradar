/**
 * View Store
 *
 * Manages the current view state for navigation between Live and Log views.
 */

import { defineStore } from 'pinia';
import { ref } from 'vue';

export type ViewType = 'live' | 'log' | 'airlines';

export const useViewStore = defineStore('view', () => {
  const currentView = ref<ViewType>('live');

  function setView(view: ViewType) {
    currentView.value = view;
  }

  function showLive() {
    currentView.value = 'live';
  }

  function showLog() {
    currentView.value = 'log';
  }

  function showAirlines() {
    currentView.value = 'airlines';
  }

  return {
    currentView,
    setView,
    showLive,
    showLog,
    showAirlines,
  };
});
