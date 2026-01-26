<template>
  <div class="login-container">
    <div class="login-card">
      <h2 class="login-title">Admin Login</h2>
      <form @submit.prevent="handleLogin">
        <div class="form-group">
          <label for="password">Password</label>
          <input
            type="password"
            id="password"
            v-model="password"
            :disabled="loading"
            placeholder="Enter admin password"
            autocomplete="current-password"
          />
        </div>
        <div v-if="error" class="error-message">
          {{ error }}
        </div>
        <button type="submit" class="login-button" :disabled="loading || !password">
          <span v-if="loading" class="spinner"></span>
          <span v-else>Login</span>
        </button>
      </form>
      <router-link to="/" class="back-link">
        <i class="bi bi-arrow-left"></i> Back to radar
      </router-link>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useRouter } from 'vue-router';
import { getAuthService } from '@/services/authService';

const router = useRouter();
const authService = getAuthService();

const password = ref('');
const loading = ref(false);
const error = ref('');

const handleLogin = async () => {
  if (!password.value) return;

  loading.value = true;
  error.value = '';

  try {
    await authService.adminLogin(password.value);
    router.push('/dashboard');
  } catch (err) {
    error.value = 'Invalid password';
    password.value = '';
  } finally {
    loading.value = false;
  }
};
</script>

<style scoped>
.login-container {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #f5f5f5;
  padding: 16px;
}

.login-card {
  background: #fff;
  border-radius: 8px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  padding: 32px;
  width: 100%;
  max-width: 360px;
}

.login-title {
  margin: 0 0 24px 0;
  font-size: 1.5rem;
  font-weight: 600;
  color: #333;
  text-align: center;
}

.form-group {
  margin-bottom: 16px;
}

.form-group label {
  display: block;
  margin-bottom: 6px;
  font-size: 0.875rem;
  font-weight: 500;
  color: #555;
}

.form-group input {
  width: 100%;
  padding: 10px 12px;
  border: 1px solid #ddd;
  border-radius: 6px;
  font-size: 1rem;
  transition: border-color 0.15s ease;
  box-sizing: border-box;
}

.form-group input:focus {
  outline: none;
  border-color: #333;
}

.form-group input:disabled {
  background: #f9f9f9;
  cursor: not-allowed;
}

.error-message {
  background: #fee;
  color: #c00;
  padding: 10px 12px;
  border-radius: 6px;
  margin-bottom: 16px;
  font-size: 0.875rem;
}

.login-button {
  width: 100%;
  padding: 12px;
  background: #333;
  color: #fff;
  border: none;
  border-radius: 6px;
  font-size: 1rem;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.15s ease;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.login-button:hover:not(:disabled) {
  background: #444;
}

.login-button:disabled {
  background: #999;
  cursor: not-allowed;
}

.spinner {
  width: 18px;
  height: 18px;
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

.back-link {
  display: block;
  text-align: center;
  margin-top: 20px;
  color: #666;
  text-decoration: none;
  font-size: 0.875rem;
  transition: color 0.15s ease;
}

.back-link:hover {
  color: #333;
}

.back-link i {
  margin-right: 4px;
}
</style>
