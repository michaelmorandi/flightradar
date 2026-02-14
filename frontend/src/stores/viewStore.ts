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
  const searchQuery = ref('');
  const listVisible = ref(window.innerWidth >= 768);

  function setView(view: ViewType) {
    currentView.value = view;
    searchQuery.value = '';
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
    searchQuery,
    listVisible,
    setView,
    showLive,
    showLog,
    showAirlines,
  };
});
