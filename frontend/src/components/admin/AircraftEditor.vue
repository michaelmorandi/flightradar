<template>
  <div class="aircraft-editor">
    <h2 class="section-title">Aircraft Editor</h2>

    <!-- Search Form -->
    <div class="search-form">
      <div class="search-input-group">
        <input
          type="text"
          v-model="searchIcao"
          placeholder="Enter ICAO24 hex code (e.g., 3C6752)"
          @keyup.enter="searchAircraft"
          :disabled="loading"
          class="search-input"
        />
        <button
          @click="searchAircraft"
          :disabled="loading || !searchIcao.trim()"
          class="search-button"
        >
          <span v-if="loading" class="spinner-small"></span>
          <span v-else>Search</span>
        </button>
      </div>
      <div v-if="searchError" class="error-message">{{ searchError }}</div>
    </div>

    <!-- Edit Form -->
    <div v-if="aircraft" class="edit-form">
      <div class="form-header">
        <h3>{{ aircraft.icao24 }}</h3>
        <span v-if="aircraft.source" class="source-badge">Source: {{ aircraft.source }}</span>
        <span v-if="isNew" class="new-badge">New Entry</span>
      </div>

      <div class="form-grid">
        <div class="form-group">
          <label for="registration">Registration</label>
          <input
            type="text"
            id="registration"
            v-model="editForm.registration"
            placeholder="e.g., D-AICD"
            :disabled="saving"
          />
        </div>

        <div class="form-group">
          <label for="icaoTypeCode">ICAO Type Code</label>
          <input
            type="text"
            id="icaoTypeCode"
            v-model="editForm.icao_type_code"
            placeholder="e.g., A320"
            :disabled="saving"
          />
        </div>

        <div class="form-group full-width">
          <label for="typeDescription">Type Description</label>
          <input
            type="text"
            id="typeDescription"
            v-model="editForm.type_description"
            placeholder="e.g., Airbus A320-214"
            :disabled="saving"
          />
        </div>

        <div class="form-group full-width">
          <label for="operator">Operator</label>
          <input
            type="text"
            id="operator"
            v-model="editForm.operator"
            placeholder="e.g., Lufthansa"
            :disabled="saving"
          />
        </div>
      </div>

      <div v-if="aircraft.icao_type_designator" class="readonly-info">
        <span class="info-label">ICAO Designator:</span>
        <span class="info-value">{{ aircraft.icao_type_designator }}</span>
      </div>

      <div v-if="aircraft.last_modified" class="readonly-info">
        <span class="info-label">Last Modified:</span>
        <span class="info-value">{{ formatDate(aircraft.last_modified) }}</span>
      </div>

      <div class="form-actions">
        <button
          @click="saveAircraft"
          :disabled="saving || !hasChanges"
          class="save-button"
        >
          <span v-if="saving" class="spinner-small"></span>
          <span v-else>Save Changes</span>
        </button>
        <button @click="resetForm" :disabled="saving" class="reset-button">
          Reset
        </button>
      </div>

      <div v-if="saveSuccess" class="success-message">Aircraft saved successfully!</div>
      <div v-if="saveError" class="error-message">{{ saveError }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import axios from 'axios';
import { config } from '@/config';

interface Aircraft {
  icao24: string;
  registration: string | null;
  icao_type_code: string | null;
  type_description: string | null;
  operator: string | null;
  icao_type_designator: string | null;
  source: string | null;
  first_created: string | null;
  last_modified: string | null;
}

interface EditForm {
  registration: string;
  icao_type_code: string;
  type_description: string;
  operator: string;
}

const searchIcao = ref('');
const aircraft = ref<Aircraft | null>(null);
const isNew = ref(false);
const loading = ref(false);
const saving = ref(false);
const searchError = ref('');
const saveError = ref('');
const saveSuccess = ref(false);

const editForm = ref<EditForm>({
  registration: '',
  icao_type_code: '',
  type_description: '',
  operator: '',
});

const originalForm = ref<EditForm>({
  registration: '',
  icao_type_code: '',
  type_description: '',
  operator: '',
});

const hasChanges = computed(() => {
  return (
    editForm.value.registration !== originalForm.value.registration ||
    editForm.value.icao_type_code !== originalForm.value.icao_type_code ||
    editForm.value.type_description !== originalForm.value.type_description ||
    editForm.value.operator !== originalForm.value.operator
  );
});

const formatDate = (isoString: string) => {
  try {
    return new Date(isoString).toLocaleString();
  } catch {
    return isoString;
  }
};

const searchAircraft = async () => {
  const icao = searchIcao.value.trim().toUpperCase();
  if (!icao) return;

  loading.value = true;
  searchError.value = '';
  saveError.value = '';
  saveSuccess.value = false;
  aircraft.value = null;
  isNew.value = false;

  try {
    const response = await axios.get<Aircraft>(
      `${config.flightApiUrl}/admin/aircraft/${icao}`,
      { withCredentials: true }
    );
    aircraft.value = response.data;
    populateForm(response.data);
  } catch (error: any) {
    if (error.response?.status === 404) {
      // Aircraft not found - allow creating new entry
      aircraft.value = {
        icao24: icao,
        registration: null,
        icao_type_code: null,
        type_description: null,
        operator: null,
        icao_type_designator: null,
        source: null,
        first_created: null,
        last_modified: null,
      };
      isNew.value = true;
      populateForm(aircraft.value);
    } else if (error.response?.status === 403) {
      searchError.value = 'Admin access required';
    } else {
      searchError.value = 'Failed to search for aircraft';
    }
  } finally {
    loading.value = false;
  }
};

const populateForm = (data: Aircraft) => {
  editForm.value = {
    registration: data.registration || '',
    icao_type_code: data.icao_type_code || '',
    type_description: data.type_description || '',
    operator: data.operator || '',
  };
  originalForm.value = { ...editForm.value };
};

const resetForm = () => {
  if (aircraft.value) {
    populateForm(aircraft.value);
  }
  saveError.value = '';
  saveSuccess.value = false;
};

const saveAircraft = async () => {
  if (!aircraft.value) return;

  saving.value = true;
  saveError.value = '';
  saveSuccess.value = false;

  try {
    const response = await axios.put<Aircraft>(
      `${config.flightApiUrl}/admin/aircraft/${aircraft.value.icao24}`,
      {
        registration: editForm.value.registration || null,
        icao_type_code: editForm.value.icao_type_code || null,
        type_description: editForm.value.type_description || null,
        operator: editForm.value.operator || null,
      },
      { withCredentials: true }
    );

    aircraft.value = response.data;
    isNew.value = false;
    populateForm(response.data);
    saveSuccess.value = true;

    // Hide success message after 3 seconds
    setTimeout(() => {
      saveSuccess.value = false;
    }, 3000);
  } catch (error: any) {
    if (error.response?.status === 403) {
      saveError.value = 'Admin access required';
    } else {
      saveError.value = 'Failed to save aircraft';
    }
  } finally {
    saving.value = false;
  }
};
</script>

<style scoped>
.aircraft-editor {
  background: #fff;
  border-radius: 12px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
  padding: 24px;
}

.section-title {
  margin: 0 0 20px 0;
  font-size: 1.25rem;
  font-weight: 600;
  color: #333;
}

.search-form {
  margin-bottom: 24px;
}

.search-input-group {
  display: flex;
  gap: 12px;
}

.search-input {
  flex: 1;
  padding: 12px 16px;
  border: 1px solid #ddd;
  border-radius: 8px;
  font-size: 1rem;
  text-transform: uppercase;
}

.search-input:focus {
  outline: none;
  border-color: #333;
}

.search-input::placeholder {
  text-transform: none;
}

.search-button {
  padding: 12px 24px;
  background: #333;
  color: #fff;
  border: none;
  border-radius: 8px;
  font-size: 1rem;
  font-weight: 500;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 100px;
  justify-content: center;
}

.search-button:hover:not(:disabled) {
  background: #444;
}

.search-button:disabled {
  background: #999;
  cursor: not-allowed;
}

.edit-form {
  border-top: 1px solid #eee;
  padding-top: 24px;
}

.form-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 20px;
}

.form-header h3 {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 600;
  font-family: monospace;
  color: #333;
}

.source-badge,
.new-badge {
  padding: 4px 10px;
  border-radius: 12px;
  font-size: 0.75rem;
  font-weight: 500;
}

.source-badge {
  background: #e8f4fd;
  color: #1976d2;
}

.new-badge {
  background: #e8f5e9;
  color: #388e3c;
}

.form-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
  margin-bottom: 16px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.form-group.full-width {
  grid-column: 1 / -1;
}

.form-group label {
  font-size: 0.875rem;
  font-weight: 500;
  color: #555;
}

.form-group input {
  padding: 10px 12px;
  border: 1px solid #ddd;
  border-radius: 6px;
  font-size: 1rem;
}

.form-group input:focus {
  outline: none;
  border-color: #333;
}

.form-group input:disabled {
  background: #f9f9f9;
}

.readonly-info {
  display: flex;
  gap: 8px;
  padding: 8px 0;
  font-size: 0.875rem;
  color: #666;
}

.info-label {
  font-weight: 500;
}

.form-actions {
  display: flex;
  gap: 12px;
  margin-top: 20px;
}

.save-button {
  padding: 12px 24px;
  background: #333;
  color: #fff;
  border: none;
  border-radius: 8px;
  font-size: 1rem;
  font-weight: 500;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 8px;
}

.save-button:hover:not(:disabled) {
  background: #444;
}

.save-button:disabled {
  background: #999;
  cursor: not-allowed;
}

.reset-button {
  padding: 12px 24px;
  background: #fff;
  color: #666;
  border: 1px solid #ddd;
  border-radius: 8px;
  font-size: 1rem;
  cursor: pointer;
}

.reset-button:hover:not(:disabled) {
  background: #f5f5f5;
  border-color: #ccc;
}

.error-message {
  margin-top: 12px;
  padding: 12px 16px;
  background: #fef2f2;
  color: #dc2626;
  border-radius: 8px;
  font-size: 0.9rem;
}

.success-message {
  margin-top: 12px;
  padding: 12px 16px;
  background: #f0fdf4;
  color: #16a34a;
  border-radius: 8px;
  font-size: 0.9rem;
}

.spinner-small {
  width: 16px;
  height: 16px;
  border: 2px solid rgba(255, 255, 255, 0.3);
  border-top-color: #fff;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 600px) {
  .form-grid {
    grid-template-columns: 1fr;
  }

  .search-input-group {
    flex-direction: column;
  }

  .form-actions {
    flex-direction: column;
  }
}
</style>
