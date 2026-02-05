<template>
  <div class="crawler-status">
    <div class="section-header">
      <h2 class="section-title">
        <i class="bi bi-gear-wide-connected"></i>
        Crawler Status
      </h2>
      <div class="header-controls">
        <div v-if="stats.sources.length > 0" class="source-toggles">
          <span class="toggles-label">Sources:</span>
          <button
            v-for="source in stats.sources"
            :key="source.name"
            class="source-toggle"
            :class="{ enabled: source.enabled, disabled: !source.enabled, toggling: togglingSource === source.name }"
            :disabled="togglingSource !== null"
            @click="toggleSource(source.name, source.enabled)"
            :title="source.enabled ? `Disable ${source.name}` : `Enable ${source.name}`"
          >
            {{ source.name }}
          </button>
        </div>
        <div class="refresh-indicator" :class="{ active: loading }">
          <i class="bi bi-arrow-clockwise" :class="{ spinning: loading }"></i>
          <span class="refresh-text">{{ refreshInterval / 1000 }}s</span>
        </div>
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
              <div class="cb-stat" v-if="cb.trip_count > 0">
                <span class="cb-stat-value trips">{{ cb.trip_count }}</span>
                <span class="cb-stat-label">Trips</span>
              </div>
            </div>
            <div v-if="cb.state === 'open' && cb.seconds_until_retry > 0" class="cb-retry-timer">
              <i class="bi bi-hourglass-split"></i>
              Retry in {{ Math.ceil(cb.seconds_until_retry) }}s
              <span class="cb-backoff-info">(backoff: {{ formatBackoff(cb.current_backoff_seconds) }})</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Recent Activity -->
      <div class="stats-section">
        <h3 class="subsection-title">Recent Activity</h3>
        <div v-if="activity.length === 0" class="no-data">
          <i class="bi bi-info-circle"></i>
          No recent activity
        </div>
        <div v-else class="activity-list">
          <div
            v-for="(item, index) in activity"
            :key="index"
            class="activity-item"
            :class="item.status"
          >
            <div class="activity-time">{{ formatTime(item.timestamp) }}</div>
            <div class="activity-icao">{{ item.icao24 }}</div>
            <div class="activity-status">
              <span class="status-badge" :class="item.status">
                {{ getStatusLabel(item.status) }}
              </span>
            </div>
            <div class="activity-details">
              <span v-if="item.registration" class="detail-reg">{{ item.registration }}</span>
              <span v-if="item.aircraft_type" class="detail-type">{{ item.aircraft_type }}</span>
              <span v-if="item.source" class="detail-source">{{ item.source }}</span>
              <span v-if="item.crawl_reason" class="detail-reason">{{ formatCrawlReason(item.crawl_reason) }}</span>
            </div>
            <button
              v-if="item.query_count >= 2"
              class="log-button"
              @click="showLogs(item.icao24)"
              title="View crawler logs"
            >
              <i class="bi bi-journal-text"></i>
            </button>
          </div>
        </div>
      </div>
    </template>

    <!-- Log Modal -->
    <div v-if="logModal.visible" class="modal-overlay" @click.self="closeLogModal">
      <div class="modal-content">
        <div class="modal-header">
          <h3>Crawler Logs: {{ logModal.icao24 }}</h3>
          <button class="modal-close" @click="closeLogModal">
            <i class="bi bi-x-lg"></i>
          </button>
        </div>
        <div class="modal-body">
          <div v-if="logModal.loading" class="modal-loading">
            <i class="bi bi-arrow-clockwise spinning"></i>
            Loading logs...
          </div>
          <div v-else-if="logModal.error" class="modal-error">
            <i class="bi bi-exclamation-triangle"></i>
            {{ logModal.error }}
          </div>
          <div v-else-if="logModal.logs.length === 0" class="modal-empty">
            <i class="bi bi-info-circle"></i>
            No logs found for this aircraft
          </div>
          <div v-else class="logs-list">
            <div v-for="(log, logIndex) in logModal.logs" :key="logIndex" class="log-entry">
              <div class="log-header">
                <span class="log-time">{{ formatTime(log.timestamp) }}</span>
                <span class="log-status" :class="log.final_status">{{ log.final_status }}</span>
                <span v-if="log.final_source" class="log-source">{{ log.final_source }}</span>
              </div>
              <div class="log-queries">
                <div v-for="(query, qIndex) in log.queries" :key="qIndex" class="query-entry">
                  <div class="query-header">
                    <span class="query-source">{{ query.source }}</span>
                    <span class="query-status" :class="query.status">{{ query.status }}</span>
                    <span class="query-duration">{{ query.duration_ms }}ms</span>
                  </div>
                  <div v-if="query.error" class="query-error">{{ query.error }}</div>
                  <div v-if="query.payload" class="query-payload">
                    <button class="payload-toggle" @click="togglePayload(logIndex, qIndex)">
                      <i :class="isPayloadExpanded(logIndex, qIndex) ? 'bi bi-chevron-down' : 'bi bi-chevron-right'"></i>
                      Response
                    </button>
                    <pre v-if="isPayloadExpanded(logIndex, qIndex)" class="payload-content">{{ JSON.stringify(query.payload, null, 2) }}</pre>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
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
  trip_count: number;
  current_backoff_seconds: number;
  seconds_until_retry: number;
}

interface SourceStatus {
  name: string;
  enabled: boolean;
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
  sources: SourceStatus[];
}

interface ActivityItem {
  icao24: string;
  timestamp: string;
  status: 'success' | 'partial' | 'not_found' | 'service_error';
  source: string | null;
  registration: string | null;
  aircraft_type: string | null;
  crawl_reason: string | null;
  query_count: number;  // Logs only exist when >= 2
}

interface CrawlerQueryLog {
  source: string;
  status: string;
  duration_ms: number;
  payload: Record<string, unknown> | null;
  error: string | null;
}

interface CrawlerLogEntry {
  icao24: string;
  timestamp: string;
  queries: CrawlerQueryLog[];
  final_status: string;
  final_source: string | null;
  query_count: number;
}

interface LogModal {
  visible: boolean;
  icao24: string;
  loading: boolean;
  error: string;
  logs: CrawlerLogEntry[];
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
  sources: [],
});

const togglingSource = ref<string | null>(null);

const activity = ref<ActivityItem[]>([]);
const loading = ref(false);
const error = ref('');
let intervalId: number | null = null;

// Log modal state
const logModal = ref<LogModal>({
  visible: false,
  icao24: '',
  loading: false,
  error: '',
  logs: [],
});
const expandedPayloads = ref<Set<string>>(new Set());

const fetchStats = async () => {
  loading.value = true;
  error.value = '';

  try {
    const [statsResponse, activityResponse] = await Promise.all([
      Axios.get<CrawlerStats>(
        `${config.flightApiUrl}/admin/crawler/stats`,
        { withCredentials: true }
      ),
      Axios.get<{ activity: ActivityItem[] }>(
        `${config.flightApiUrl}/admin/crawler/activity`,
        { withCredentials: true }
      ),
    ]);

    if (statsResponse.status >= 200 && statsResponse.status < 300) {
      stats.value = statsResponse.data;
    }
    if (activityResponse.status >= 200 && activityResponse.status < 300) {
      activity.value = activityResponse.data.activity;
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

const formatTime = (isoString: string) => {
  try {
    const date = new Date(isoString);
    return date.toLocaleTimeString();
  } catch {
    return isoString;
  }
};

const getStatusLabel = (status: string) => {
  switch (status) {
    case 'success': return 'Found';
    case 'partial': return 'Partial';
    case 'not_found': return 'Not Found';
    case 'service_error': return 'Error';
    default: return status;
  }
};

const formatBackoff = (seconds: number) => {
  if (seconds >= 60) {
    const mins = Math.floor(seconds / 60);
    return `${mins}m`;
  }
  return `${seconds}s`;
};

const formatCrawlReason = (reason: string) => {
  switch (reason) {
    case 'not_in_db': return 'New';
    case 'no_timestamp': return 'No timestamp';
    case 'incomplete_stale': return 'Incomplete';
    case 'stale': return 'Stale';
    case 'unknown': return 'Unknown';
    default: return reason;
  }
};

const toggleSource = async (sourceName: string, currentEnabled: boolean) => {
  togglingSource.value = sourceName;
  try {
    await Axios.post(
      `${config.flightApiUrl}/admin/crawler/sources/${encodeURIComponent(sourceName)}/toggle`,
      { enabled: !currentEnabled },
      { withCredentials: true }
    );
    // Update local state optimistically
    const source = stats.value.sources.find(s => s.name === sourceName);
    if (source) {
      source.enabled = !currentEnabled;
    }
  } catch (err) {
    console.error('Error toggling source:', err);
  } finally {
    togglingSource.value = null;
  }
};

const showLogs = async (icao24: string) => {
  logModal.value = {
    visible: true,
    icao24,
    loading: true,
    error: '',
    logs: [],
  };
  expandedPayloads.value.clear();

  try {
    const response = await Axios.get<{ icao24: string; logs: CrawlerLogEntry[] }>(
      `${config.flightApiUrl}/admin/crawler/logs/${encodeURIComponent(icao24)}`,
      { withCredentials: true }
    );
    logModal.value.logs = response.data.logs;
  } catch (err: any) {
    logModal.value.error = err.response?.data?.detail || 'Failed to load logs';
    console.error('Error fetching crawler logs:', err);
  } finally {
    logModal.value.loading = false;
  }
};

const closeLogModal = () => {
  logModal.value.visible = false;
};

const togglePayload = (logIndex: number, queryIndex: number) => {
  const key = `${logIndex}-${queryIndex}`;
  if (expandedPayloads.value.has(key)) {
    expandedPayloads.value.delete(key);
  } else {
    expandedPayloads.value.add(key);
  }
};

const isPayloadExpanded = (logIndex: number, queryIndex: number) => {
  return expandedPayloads.value.has(`${logIndex}-${queryIndex}`);
};

const startPolling = () => {
  if (intervalId === null) {
    intervalId = window.setInterval(fetchStats, refreshInterval);
  }
};

const stopPolling = () => {
  if (intervalId !== null) {
    clearInterval(intervalId);
    intervalId = null;
  }
};

const handleVisibilityChange = () => {
  if (document.hidden) {
    stopPolling();
  } else {
    fetchStats();
    startPolling();
  }
};

onMounted(() => {
  fetchStats();
  startPolling();
  document.addEventListener('visibilitychange', handleVisibilityChange);
});

onUnmounted(() => {
  stopPolling();
  document.removeEventListener('visibilitychange', handleVisibilityChange);
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
  flex-wrap: wrap;
  gap: 12px;
}

.header-controls {
  display: flex;
  align-items: center;
  gap: 16px;
}

.source-toggles {
  display: flex;
  align-items: center;
  gap: 6px;
}

.toggles-label {
  font-size: 0.75rem;
  color: #888;
  margin-right: 2px;
}

.source-toggle {
  font-size: 0.7rem;
  padding: 3px 8px;
  border-radius: 10px;
  border: 1px solid #ddd;
  background: #f5f5f5;
  color: #666;
  cursor: pointer;
  transition: all 0.15s ease;
  white-space: nowrap;
}

.source-toggle:hover:not(:disabled) {
  border-color: #bbb;
}

.source-toggle.enabled {
  background: #e8f5e9;
  border-color: #a5d6a7;
  color: #2e7d32;
}

.source-toggle.disabled {
  background: #fafafa;
  border-color: #e0e0e0;
  color: #9e9e9e;
  text-decoration: line-through;
}

.source-toggle.toggling {
  opacity: 0.6;
  cursor: wait;
}

.source-toggle:disabled {
  cursor: not-allowed;
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

.cb-stat-value.trips {
  color: #9c27b0;
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

.cb-backoff-info {
  color: #888;
  font-size: 0.75rem;
}

/* Activity List */
.activity-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 400px;
  overflow-y: auto;
}

.activity-item {
  display: grid;
  grid-template-columns: 80px 80px 90px 1fr auto;
  gap: 12px;
  align-items: center;
  padding: 10px 12px;
  background: #f8f9fa;
  border-radius: 6px;
  font-size: 0.85rem;
  border-left: 3px solid #4caf50;
}

.activity-item.partial {
  border-left-color: #ff9800;
}

.activity-item.not_found {
  border-left-color: #9e9e9e;
}

.activity-item.service_error {
  border-left-color: #f44336;
}

.activity-time {
  color: #888;
  font-family: monospace;
  font-size: 0.8rem;
}

.activity-icao {
  font-family: monospace;
  font-weight: 600;
  color: #333;
}

.status-badge {
  display: inline-block;
  padding: 2px 8px;
  border-radius: 10px;
  font-size: 0.7rem;
  font-weight: 500;
  text-transform: uppercase;
}

.status-badge.success {
  background: #e8f5e9;
  color: #2e7d32;
}

.status-badge.partial {
  background: #fff3e0;
  color: #ef6c00;
}

.status-badge.not_found {
  background: #f5f5f5;
  color: #757575;
}

.status-badge.service_error {
  background: #ffebee;
  color: #c62828;
}

.activity-details {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
}

.detail-reg {
  font-weight: 500;
  color: #333;
}

.detail-type {
  color: #666;
}

.detail-source {
  color: #1976d2;
  font-size: 0.75rem;
  background: #e3f2fd;
  padding: 1px 6px;
  border-radius: 4px;
}

.detail-reason {
  color: #7b1fa2;
  font-size: 0.7rem;
  background: #f3e5f5;
  padding: 1px 6px;
  border-radius: 4px;
  font-style: italic;
}

/* Log Button */
.log-button {
  background: transparent;
  border: 1px solid #ddd;
  border-radius: 4px;
  padding: 4px 8px;
  cursor: pointer;
  color: #666;
  transition: all 0.15s;
}

.log-button:hover {
  background: #e3f2fd;
  border-color: #1976d2;
  color: #1976d2;
}

/* Modal Styles */
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  padding: 20px;
}

.modal-content {
  background: #fff;
  border-radius: 12px;
  max-width: 800px;
  width: 100%;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px;
  border-bottom: 1px solid #eee;
}

.modal-header h3 {
  margin: 0;
  font-size: 1.1rem;
  font-weight: 600;
  color: #333;
}

.modal-close {
  background: transparent;
  border: none;
  font-size: 1.2rem;
  color: #666;
  cursor: pointer;
  padding: 4px;
  line-height: 1;
}

.modal-close:hover {
  color: #333;
}

.modal-body {
  padding: 20px;
  overflow-y: auto;
  flex: 1;
}

.modal-loading,
.modal-error,
.modal-empty {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 20px;
  justify-content: center;
  color: #666;
}

.modal-error {
  color: #c62828;
  background: #ffebee;
  border-radius: 8px;
}

/* Log Entry Styles */
.logs-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.log-entry {
  background: #f8f9fa;
  border-radius: 8px;
  padding: 12px;
}

.log-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
  padding-bottom: 8px;
  border-bottom: 1px solid #eee;
}

.log-time {
  font-family: monospace;
  font-size: 0.85rem;
  color: #666;
}

.log-status {
  font-size: 0.75rem;
  padding: 2px 8px;
  border-radius: 10px;
  font-weight: 500;
  text-transform: uppercase;
}

.log-status.success {
  background: #e8f5e9;
  color: #2e7d32;
}

.log-status.partial {
  background: #fff3e0;
  color: #ef6c00;
}

.log-status.not_found {
  background: #f5f5f5;
  color: #757575;
}

.log-status.service_error {
  background: #ffebee;
  color: #c62828;
}

.log-source {
  font-size: 0.8rem;
  color: #1976d2;
  background: #e3f2fd;
  padding: 2px 8px;
  border-radius: 4px;
}

.log-queries {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.query-entry {
  background: #fff;
  border-radius: 6px;
  padding: 10px;
  border: 1px solid #eee;
}

.query-header {
  display: flex;
  align-items: center;
  gap: 10px;
}

.query-source {
  font-weight: 500;
  color: #333;
}

.query-status {
  font-size: 0.7rem;
  padding: 2px 6px;
  border-radius: 8px;
  text-transform: uppercase;
}

.query-status.success,
.query-status.partial_data {
  background: #e8f5e9;
  color: #2e7d32;
}

.query-status.not_found {
  background: #f5f5f5;
  color: #757575;
}

.query-status.service_error {
  background: #ffebee;
  color: #c62828;
}

.query-status.skipped_circuit_breaker {
  background: #fff3e0;
  color: #ef6c00;
}

.query-duration {
  font-size: 0.75rem;
  color: #888;
  font-family: monospace;
}

.query-error {
  margin-top: 8px;
  padding: 8px;
  background: #ffebee;
  border-radius: 4px;
  color: #c62828;
  font-size: 0.8rem;
}

.query-payload {
  margin-top: 8px;
}

.payload-toggle {
  background: transparent;
  border: 1px solid #ddd;
  border-radius: 4px;
  padding: 4px 8px;
  font-size: 0.75rem;
  cursor: pointer;
  color: #666;
  display: flex;
  align-items: center;
  gap: 4px;
}

.payload-toggle:hover {
  background: #f5f5f5;
}

.payload-content {
  margin-top: 8px;
  padding: 10px;
  background: #f5f5f5;
  border-radius: 4px;
  font-size: 0.75rem;
  overflow-x: auto;
  max-height: 200px;
  overflow-y: auto;
}

/* Responsive */
@media (max-width: 600px) {
  .queue-stats-grid {
    grid-template-columns: 1fr;
  }

  .section-header {
    flex-direction: column;
    align-items: flex-start;
  }

  .header-controls {
    width: 100%;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }

  .source-toggles {
    flex-wrap: wrap;
  }

  .activity-item {
    grid-template-columns: 1fr 1fr;
    grid-template-rows: auto auto;
  }

  .activity-details {
    grid-column: 1 / -1;
  }
}
</style>
