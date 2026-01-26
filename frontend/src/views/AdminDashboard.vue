<template>
  <div class="dashboard-container">
    <div class="dashboard-header">
      <h1 class="dashboard-title">Admin Dashboard</h1>
      <button class="logout-button" @click="handleLogout" :disabled="loggingOut">
        <i class="bi bi-box-arrow-right"></i>
        <span v-if="loggingOut">Logging out...</span>
        <span v-else>Logout</span>
      </button>
    </div>

    <div class="stats-grid">
      <div class="stat-card">
        <div class="stat-icon">
          <i class="bi bi-airplane"></i>
        </div>
        <div class="stat-content">
          <div class="stat-label">Flights in Database</div>
          <div class="stat-value" v-if="!loading">{{ flightCount.toLocaleString() }}</div>
          <div class="stat-value loading" v-else>
            <span class="spinner-small"></span>
          </div>
        </div>
      </div>
    </div>

    <div v-if="error" class="error-banner">
      <i class="bi bi-exclamation-triangle"></i>
      {{ error }}
    </div>

    <div class="actions">
      <router-link to="/" class="action-link">
        <i class="bi bi-map"></i> View Live Radar
      </router-link>
      <router-link to="/flightlog" class="action-link">
        <i class="bi bi-list-ul"></i> View Flight Log
      </router-link>
    </div>

    <div class="editor-section">
      <AircraftEditor />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import Axios from 'axios';
import { config } from '@/config';
import { getAuthService } from '@/services/authService';
import AircraftEditor from '@/components/admin/AircraftEditor.vue';

const router = useRouter();
const authService = getAuthService();

const flightCount = ref(0);
const loading = ref(true);
const error = ref('');
const loggingOut = ref(false);

const fetchStats = async () => {
  loading.value = true;
  error.value = '';

  try {
    const response = await Axios.get(`${config.flightApiUrl}/admin/stats`, {
      withCredentials: true,
    });

    if (response.status >= 200 && response.status < 300) {
      flightCount.value = response.data.flight_count;
    }
  } catch (err) {
    error.value = 'Failed to load dashboard statistics';
    console.error('Error fetching admin stats:', err);
  } finally {
    loading.value = false;
  }
};

const handleLogout = async () => {
  loggingOut.value = true;
  try {
    await authService.adminLogout();
    router.push('/');
  } catch (err) {
    console.error('Logout error:', err);
    router.push('/');
  }
};

onMounted(() => {
  // Check if user is admin, redirect if not
  if (!authService.isAdmin()) {
    router.push('/dashboard/login');
    return;
  }
  fetchStats();
});
</script>

<style scoped>
.dashboard-container {
  max-width: 1000px;
  margin: 0 auto;
  padding: 24px;
}

.dashboard-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 32px;
}

.dashboard-title {
  margin: 0;
  font-size: 1.75rem;
  font-weight: 600;
  color: #333;
}

.logout-button {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  background: #fff;
  border: 1px solid #ddd;
  border-radius: 6px;
  color: #666;
  font-size: 0.9rem;
  cursor: pointer;
  transition: all 0.15s ease;
}

.logout-button:hover:not(:disabled) {
  background: #f5f5f5;
  border-color: #ccc;
  color: #333;
}

.logout-button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: 20px;
  margin-bottom: 32px;
}

.stat-card {
  background: #fff;
  border-radius: 12px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
  padding: 24px;
  display: flex;
  align-items: center;
  gap: 20px;
}

.stat-icon {
  width: 56px;
  height: 56px;
  background: #f0f0f0;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.stat-icon i {
  font-size: 1.5rem;
  color: #333;
}

.stat-content {
  flex: 1;
}

.stat-label {
  font-size: 0.875rem;
  color: #666;
  margin-bottom: 4px;
}

.stat-value {
  font-size: 2rem;
  font-weight: 600;
  color: #333;
}

.stat-value.loading {
  display: flex;
  align-items: center;
}

.spinner-small {
  width: 24px;
  height: 24px;
  border: 3px solid #eee;
  border-top-color: #333;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.error-banner {
  background: #fee;
  color: #c00;
  padding: 16px 20px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 32px;
}

.error-banner i {
  font-size: 1.25rem;
}

.actions {
  display: flex;
  gap: 16px;
  flex-wrap: wrap;
}

.action-link {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 20px;
  background: #fff;
  border: 1px solid #ddd;
  border-radius: 8px;
  color: #333;
  text-decoration: none;
  font-size: 0.9rem;
  transition: all 0.15s ease;
}

.action-link:hover {
  background: #f5f5f5;
  border-color: #ccc;
}

.action-link i {
  font-size: 1rem;
}

.editor-section {
  margin-top: 32px;
}
</style>
