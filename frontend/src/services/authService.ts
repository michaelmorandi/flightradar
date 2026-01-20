/**
 * Authentication Service
 *
 * Handles authentication via fastapi-users JWT cookie flow.
 * Implements automatic token refresh to maintain session without user intervention.
 *
 * Authentication Flow:
 * 1. POST /auth/jwt/login with email/password
 * 2. Receive JWT as HTTP-only cookie
 * 3. Auto-refresh before expiration
 */

import axios, { type AxiosInstance } from 'axios';
import { config } from '@/config';

/** Refresh token 2 minutes before expiry */
const REFRESH_BUFFER_MS = 2 * 60 * 1000;

/** Token lifetime in seconds (must match backend) */
const TOKEN_LIFETIME_SECONDS = 900; // 15 minutes

/** Anonymous user email */
const ANONYMOUS_EMAIL = 'anonymous@system.local';

export class AuthService {
  private axios: AxiosInstance;
  private apiUrl: string;
  private clientSecret: string;
  private refreshTimer: number | null = null;
  private isAuthenticated = false;

  constructor() {
    this.axios = axios.create({
      withCredentials: true, // Required for cookies
    });

    this.apiUrl = config.flightApiUrl || '';
    this.clientSecret = config.clientSecret || '';

    if (!this.clientSecret) {
      console.warn('CLIENT_SECRET not configured - authentication will fail');
    }
  }

  /**
   * Check if authentication is initialized.
   */
  public isAuth(): boolean {
    return this.isAuthenticated;
  }

  /**
   * Initialize authentication with anonymous login.
   *
   * This should be called when the application starts.
   * On success, JWT cookie is set and auto-refresh is scheduled.
   *
   * @throws Error if authentication fails
   */
  public async initialize(): Promise<void> {
    if (!this.apiUrl) {
      throw new Error('Flight API URL not configured');
    }

    if (!this.clientSecret) {
      throw new Error('Client secret not configured');
    }

    console.log('[Auth] Initializing authentication...');

    try {
      await this.login(ANONYMOUS_EMAIL, this.clientSecret);

      this.isAuthenticated = true;
      console.log('[Auth] Authentication successful');

      // Schedule token refresh
      this.scheduleRefresh(TOKEN_LIFETIME_SECONDS);
    } catch (error) {
      this.isAuthenticated = false;
      console.error('[Auth] Authentication failed:', error);
      throw new Error('Failed to authenticate with API');
    }
  }

  /**
   * Login with email and password.
   *
   * @param email User email
   * @param password User password
   */
  public async login(email: string, password: string): Promise<void> {
    const formData = new URLSearchParams();
    formData.append('username', email); // fastapi-users uses 'username' field
    formData.append('password', password);

    const response = await this.axios.post(`${this.apiUrl}/auth/jwt/login`, formData, {
      headers: {
        'Content-Type': 'application/x-www-form-urlencoded',
      },
    });

    if (response.status !== 200 && response.status !== 204) {
      throw new Error(`Login failed: ${response.statusText}`);
    }
  }

  /**
   * Refresh the authentication token by re-logging in.
   *
   * This is called automatically before expiration.
   */
  public async refresh(): Promise<void> {
    if (!this.apiUrl || !this.clientSecret) {
      throw new Error('Authentication not properly configured');
    }

    console.log('[Auth] Refreshing token...');

    try {
      await this.login(ANONYMOUS_EMAIL, this.clientSecret);
      console.log('[Auth] Token refreshed successfully');
      this.scheduleRefresh(TOKEN_LIFETIME_SECONDS);
    } catch (error) {
      this.isAuthenticated = false;
      console.error('[Auth] Token refresh failed:', error);
      throw error;
    }
  }

  /**
   * Logout and clear authentication.
   *
   * Clears the JWT cookie and cancels auto-refresh.
   */
  public async logout(): Promise<void> {
    console.log('[Auth] Logging out...');

    if (this.refreshTimer !== null) {
      clearTimeout(this.refreshTimer);
      this.refreshTimer = null;
    }

    try {
      await this.axios.post(`${this.apiUrl}/auth/jwt/logout`);
    } catch (error) {
      console.error('[Auth] Logout request failed:', error);
    }

    this.isAuthenticated = false;
    console.log('[Auth] Logged out');
  }

  /**
   * Schedule automatic token refresh.
   *
   * Refreshes the token REFRESH_BUFFER_MS before it expires.
   *
   * @private
   */
  private scheduleRefresh(expiresInSeconds: number): void {
    if (this.refreshTimer !== null) {
      clearTimeout(this.refreshTimer);
    }

    const expiresInMs = expiresInSeconds * 1000;
    const refreshInMs = Math.max(expiresInMs - REFRESH_BUFFER_MS, 0);

    console.log(
      `[Auth] Token expires in ${expiresInSeconds}s, will refresh in ${refreshInMs / 1000}s`
    );

    this.refreshTimer = window.setTimeout(() => {
      this.refresh().catch((error) => {
        console.error('[Auth] Scheduled refresh failed:', error);
      });
    }, refreshInMs);
  }
}

// Global singleton instance
let authServiceInstance: AuthService | null = null;

/**
 * Get the global AuthService singleton.
 *
 * @returns The global AuthService instance
 */
export function getAuthService(): AuthService {
  if (!authServiceInstance) {
    authServiceInstance = new AuthService();
  }
  return authServiceInstance;
}
