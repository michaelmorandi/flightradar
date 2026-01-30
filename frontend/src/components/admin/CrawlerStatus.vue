<template>
  <div class="crawler-status">
    <div class="section-header">
      <h2 class="section-title">
        <i class="bi bi-gear-wide-connected"></i>
        Crawler Status
      </h2>
      <div class="refresh-indicator" :class="{ active: loading }">
        <i class="bi bi-arrow-clockwise" :class="{ spinning: loading }"></i>
        <span class="refresh-text">Auto-refresh: {{ refreshInterval / 1000 }}s</span>
      </div>
    </div>

    <div v-if="error" class="error-message">
      <i class="bi bi-exclamation-triangle"></i>
      {{ error }}
    </div>

    <div v-if="!stats.enabled" class="disabled-notice">
      <i class="bi bi-pause-circle"></i>
      <span>Crawler is disabled. Set <code>UNKNOWN_AIRCRAFT_CRAWLING=true</code> to enable.</span>
    </div>

    <template v-else>
      <!-- Queue Statistics -->
      <div class="stats-section">
        <h3 class="subsection-title">Processing Queue</h3>
        <div class="queue-stats-grid">
          <div class="queue-stat">
            <div class="stat-number">{{ stats.queue_total }}</div>
            <div class="stat-label">Total in Queue</div>
          </div>
          <div class="queue-stat">
            <div class="stat-number pending">{{ stats.queue_pending }}</div>
            <div class="stat-label">Pending</div>
          </div>
          <div class="queue-stat">
            <div class="stat-number in-progress">{{ stats.queue_in_progress }}</div>
            <div class="stat-label">In Progress</div>
          </div>
        </div>
      </div>

      <!-- Failure Statistics -->
      <div class="stats-section">
        <h3 class="subsection-title">Failure Tracking</h3>
        <div class="failure-stats-grid">
          <div class="failure-stat">
            <div class="stat-icon not-found">
              <i class="bi bi-search"></i>
            </div>
            <div class="stat-info">
              <div class="stat-number">{{ stats.not_found_failures }}</div>
              <div class="stat-label">Not Found</div>
              <div class="stat-description">Aircraft not in external sources</div>
            </div>
          </div>
          <div class="failure-stat">
            <div class="stat-icon service-error">
              <i class="bi bi-cloud-slash"></i>
            </div>
            <div class="stat-info">
              <div class="stat-number">{{ stats.service_error_failures }}</div>
              <div class="stat-label">Service Errors</div>
              <div class="stat-description">Temporary failures (will retry)</div>
            </div>
          </div>
          <div class="failure-stat">
            <div class="stat-icon exhausted">
              <i class="bi bi-x-circle"></i>
            </div>
            <div class="stat-info">
              <div class="stat-number">{{ stats.max_attempts_reached }}</div>
              <div class="stat-label">Max Attempts</div>
              <div class="stat-description">Exhausted all retries</div>
            </div>
          </div>
        </div>
      </div>

      <!-- Circuit Breakers -->
      <div class="stats-section">
        <h3 class="subsection-title">External Services Health</h3>
        <div v-if="Object.keys(stats.circuit_breakers).length === 0" class="no-data">
          <i class="bi bi-info-circle"></i>
          No service activity recorded yet
        </div>
        <div v-else class="circuit-breakers-grid">
          <div
            v-for="(cb, sourceName) in stats.circuit_breakers"
            :key="sourceName"
            class="circuit-breaker-card"
            :class="cb.state"
          >
            <div class="cb-header">
              <span class="cb-name">{{ sourceName }}</span>
              <span class="cb-state-badge" :class="cb.state">
                {{ cb.state === 'closed' ? 'Healthy' : cb.state === 'open' ? 'Down' : 'Testing' }}
              </span>
            </div>
            <div class="cb-stats">
              <div class="cb-stat">
                <span class="cb-stat-value success">{{ cb.total_successes }}</span>
                <span class="cb-stat-label">Success</span>
              </div>
              <div class="cb-stat">
                <span class="cb-stat-value failure">{{ cb.total_failures }}</span>
                <span class="cb-stat-label">Failures</span>
              </div>
              <div class="cb-stat">
                <span class="cb-stat-value consecutive">{{ cb.consecutive_failures }}</span>
                <span class="cb-stat-label">Consecutive</span>
              </div>
            </div>
            <div v-if="cb.state === 'open' && cb.seconds_until_retry > 0" class="cb-retry-timer">
              <i class="bi bi-hourglass-split"></i>
              Retry in {{ Math.ceil(cb.seconds_until_retry) }}s
            </div>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import Axios from 'axios';
import { config } from '@/config';

interface CircuitBreakerStats {
  state: string;
  consecutive_failures: number;
  total_failures: number;
  total_successes: number;
  seconds_until_retry: number;
}

interface CrawlerStats {
  enabled: boolean;
  queue_total: number;
  queue_pending: number;
  queue_in_progress: number;
  not_found_failures: number;
  service_error_failures: number;
  max_attempts_reached: number;
  circuit_breakers: Record<string, CircuitBreakerStats>;
}

const refreshInterval = 5000; // 5 seconds

const stats = ref<CrawlerStats>({
  enabled: false,
  queue_total: 0,
  queue_pending: 0,
  queue_in_progress: 0,
  not_found_failures: 0,
  service_error_failures: 0,
  max_attempts_reached: 0,
  circuit_breakers: {},
});

const loading = ref(false);
const error = ref('');
let intervalId: number | null = null;

const fetchStats = async () => {
  loading.value = true;
  error.value = '';

  try {
    const response = await Axios.get<CrawlerStats>(
      `${config.flightApiUrl}/admin/crawler/stats`,
      { withCredentials: true }
    );

    if (response.status >= 200 && response.status < 300) {
      stats.value = response.data;
    }
  } catch (err: any) {
    if (err.response?.status === 403) {
      error.value = 'Admin access required';
    } else {
      error.value = 'Failed to load crawler statistics';
    }
    console.error('Error fetching crawler stats:', err);
  } finally {
    loading.value = false;
  }
};

onMounted(() => {
  fetchStats();
  intervalId = window.setInterval(fetchStats, refreshInterval);
});

onUnmounted(() => {
  if (intervalId !== null) {
    clearInterval(intervalId);
  }
});
</script>

<style scoped>
.crawler-status {
  background: #fff;
  border-radius: 12px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
  padding: 24px;
}

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 24px;
}

.section-title {
  margin: 0;
  font-size: 1.25rem;
  font-weight: 600;
  color: #333;
  display: flex;
  align-items: center;
  gap: 10px;
}

.section-title i {
  color: #666;
}

.refresh-indicator {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 0.8rem;
  color: #888;
}

.refresh-indicator i {
  font-size: 1rem;
  transition: color 0.2s;
}

.refresh-indicator.active i {
  color: #2196f3;
}

.refresh-indicator i.spinning {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.refresh-text {
  font-family: monospace;
}

.error-message {
  background: #fee;
  color: #c00;
  padding: 12px 16px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 16px;
}

.disabled-notice {
  background: #f5f5f5;
  color: #666;
  padding: 16px 20px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  gap: 12px;
}

.disabled-notice code {
  background: #e0e0e0;
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 0.85em;
}

.stats-section {
  margin-bottom: 24px;
}

.stats-section:last-child {
  margin-bottom: 0;
}

.subsection-title {
  font-size: 0.9rem;
  font-weight: 600;
  color: #666;
  margin: 0 0 16px 0;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

/* Queue Stats */
.queue-stats-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 16px;
}

.queue-stat {
  background: #f8f9fa;
  border-radius: 8px;
  padding: 16px;
  text-align: center;
}

.queue-stat .stat-number {
  font-size: 2rem;
  font-weight: 600;
  color: #333;
}

.queue-stat .stat-number.pending {
  color: #ff9800;
}

.queue-stat .stat-number.in-progress {
  color: #2196f3;
}

.queue-stat .stat-label {
  font-size: 0.85rem;
  color: #666;
  margin-top: 4px;
}

/* Failure Stats */
.failure-stats-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 16px;
}

.failure-stat {
  display: flex;
  align-items: center;
  gap: 16px;
  background: #f8f9fa;
  border-radius: 8px;
  padding: 16px;
}

.failure-stat .stat-icon {
  width: 48px;
  height: 48px;
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.failure-stat .stat-icon i {
  font-size: 1.25rem;
  color: #fff;
}

.failure-stat .stat-icon.not-found {
  background: #9e9e9e;
}

.failure-stat .stat-icon.service-error {
  background: #ff9800;
}

.failure-stat .stat-icon.exhausted {
  background: #f44336;
}

.failure-stat .stat-info {
  flex: 1;
}

.failure-stat .stat-number {
  font-size: 1.5rem;
  font-weight: 600;
  color: #333;
}

.failure-stat .stat-label {
  font-size: 0.9rem;
  color: #333;
  font-weight: 500;
}

.failure-stat .stat-description {
  font-size: 0.75rem;
  color: #888;
  margin-top: 2px;
}

/* Circuit Breakers */
.no-data {
  background: #f8f9fa;
  color: #888;
  padding: 16px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  gap: 10px;
}

.circuit-breakers-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: 16px;
}

.circuit-breaker-card {
  background: #f8f9fa;
  border-radius: 8px;
  padding: 16px;
  border-left: 4px solid #4caf50;
}

.circuit-breaker-card.open {
  border-left-color: #f44336;
  background: #fff5f5;
}

.circuit-breaker-card.half_open {
  border-left-color: #ff9800;
  background: #fff8e1;
}

.cb-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.cb-name {
  font-weight: 600;
  color: #333;
}

.cb-state-badge {
  font-size: 0.75rem;
  padding: 4px 10px;
  border-radius: 12px;
  font-weight: 500;
}

.cb-state-badge.closed {
  background: #e8f5e9;
  color: #2e7d32;
}

.cb-state-badge.open {
  background: #ffebee;
  color: #c62828;
}

.cb-state-badge.half_open {
  background: #fff3e0;
  color: #ef6c00;
}

.cb-stats {
  display: flex;
  gap: 16px;
}

.cb-stat {
  display: flex;
  flex-direction: column;
  align-items: center;
}

.cb-stat-value {
  font-size: 1.25rem;
  font-weight: 600;
}

.cb-stat-value.success {
  color: #4caf50;
}

.cb-stat-value.failure {
  color: #f44336;
}

.cb-stat-value.consecutive {
  color: #ff9800;
}

.cb-stat-label {
  font-size: 0.7rem;
  color: #888;
  text-transform: uppercase;
}

.cb-retry-timer {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid #eee;
  font-size: 0.85rem;
  color: #f44336;
  display: flex;
  align-items: center;
  gap: 8px;
}

/* Responsive */
@media (max-width: 600px) {
  .queue-stats-grid {
    grid-template-columns: 1fr;
  }

  .section-header {
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }
}
</style>
