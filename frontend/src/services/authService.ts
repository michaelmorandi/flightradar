/**
 * Authentication service for the Rust backend.
 *
 * Auth model:
 *   - Encrypted HTTP-only cookie `fr_session` set by the server.
 *   - JSON login (admin) and credential-free anonymous login.
 *   - `expires_at` returned on login drives the refresh timer; we
 *     refresh `REFRESH_BUFFER_MS` before expiry by re-running anonymous
 *     login (anonymous) or by calling `/auth/me` (admin, since we don't
 *     keep the password in memory).
 */

import axios, { type AxiosInstance } from 'axios';
import { config } from '@/config';
import type { LoginResponse, UserInfo } from '@/model/backendModel';

/** Refresh token 2 minutes before expiry */
const REFRESH_BUFFER_MS = 2 * 60 * 1000;

/** Fallback token lifetime when the server doesn't report one. */
const FALLBACK_LIFETIME_SECONDS = 900;

export class AuthService {
  private axios: AxiosInstance;
  private apiUrl: string;
  private refreshTimer: number | null = null;
  private isAuthenticated = false;
  private _isAdmin = false;

  constructor() {
    this.axios = axios.create({ withCredentials: true });
    this.apiUrl = config.flightApiUrl || '';
  }

  public isAuth(): boolean {
    return this.isAuthenticated;
  }

  public isAdmin(): boolean {
    return this._isAdmin;
  }

  private async checkSession(): Promise<UserInfo | null> {
    try {
      const response = await this.axios.get<UserInfo>(`${this.apiUrl}/auth/me`);
      return response.status === 200 ? response.data : null;
    } catch {
      return null;
    }
  }

  /**
   * Initialise auth. Picks up an existing session if the cookie is
   * still valid; otherwise logs in anonymously.
   */
  public async initialize(): Promise<void> {
    if (!this.apiUrl) {
      throw new Error('Flight API URL not configured');
    }
    console.log('[Auth] Initialising authentication...');

    const existing = await this.checkSession();
    if (existing) {
      console.log(`[Auth] Existing session found: ${existing.role}`);
      this.isAuthenticated = true;
      this._isAdmin = existing.is_admin;
      // We have no `expires_at` for a recovered cookie; refresh on the
      // safe-side schedule.
      this.scheduleRefresh(FALLBACK_LIFETIME_SECONDS);
      return;
    }

    try {
      const expiresIn = await this.anonymousLogin();
      this.isAuthenticated = true;
      this._isAdmin = false;
      console.log('[Auth] Authenticated anonymously');
      this.scheduleRefresh(expiresIn);
    } catch (error) {
      this.isAuthenticated = false;
      this._isAdmin = false;
      console.error('[Auth] Authentication failed:', error);
      throw new Error('Failed to authenticate with API');
    }
  }

  /** POST /auth/anonymous. Returns the seconds until expiry. */
  private async anonymousLogin(): Promise<number> {
    const response = await this.axios.post<LoginResponse>(
      `${this.apiUrl}/auth/anonymous`,
    );
    if (response.status !== 200) {
      throw new Error(`Anonymous login failed: ${response.statusText}`);
    }
    return secondsUntil(response.data.expires_at);
  }

  /**
   * Email/password login. The Rust backend authorises admin or regular
   * users through the same endpoint; admin role is read from the
   * returned `user.is_admin` flag.
   */
  public async login(email: string, password: string): Promise<UserInfo> {
    const response = await this.axios.post<LoginResponse>(
      `${this.apiUrl}/auth/login`,
      { email, password },
    );
    if (response.status !== 200) {
      throw new Error(`Login failed: ${response.statusText}`);
    }
    this.isAuthenticated = true;
    this._isAdmin = response.data.user.is_admin;
    this.scheduleRefresh(secondsUntil(response.data.expires_at));
    return response.data.user;
  }

  /**
   * Convenience wrapper preserved for the AdminLogin.vue UI. Same
   * endpoint as `login()`; the admin email is the operator-configured
   * `ADMIN_EMAIL` on the backend.
   */
  public async adminLogin(email: string, password: string): Promise<void> {
    const user = await this.login(email, password);
    if (!user.is_admin) {
      throw new Error('Account is not an admin');
    }
  }

  /**
   * Log out the current session and fall back to anonymous.
   */
  public async adminLogout(): Promise<void> {
    if (this.refreshTimer !== null) {
      clearTimeout(this.refreshTimer);
      this.refreshTimer = null;
    }
    this._isAdmin = false;
    try {
      await this.axios.post(`${this.apiUrl}/auth/logout`);
    } catch (error) {
      console.error('[Auth] Logout request failed:', error);
    }
    try {
      const expiresIn = await this.anonymousLogin();
      this.isAuthenticated = true;
      this.scheduleRefresh(expiresIn);
    } catch (error) {
      this.isAuthenticated = false;
      console.error('[Auth] Fallback to anonymous auth failed:', error);
    }
  }

  /** Hard logout — no fallback. */
  public async logout(): Promise<void> {
    if (this.refreshTimer !== null) {
      clearTimeout(this.refreshTimer);
      this.refreshTimer = null;
    }
    try {
      await this.axios.post(`${this.apiUrl}/auth/logout`);
    } catch (error) {
      console.error('[Auth] Logout request failed:', error);
    }
    this.isAuthenticated = false;
    this._isAdmin = false;
  }

  /**
   * Refresh the session. Admins (no password in memory) bounce through
   * /auth/me; anonymous sessions issue a fresh anonymous login.
   */
  public async refresh(): Promise<void> {
    if (!this.apiUrl) {
      throw new Error('Authentication not configured');
    }
    try {
      if (this._isAdmin) {
        const session = await this.checkSession();
        if (session?.is_admin) {
          this.scheduleRefresh(FALLBACK_LIFETIME_SECONDS);
        } else {
          console.log('[Auth] Admin session expired, falling back to anonymous');
          this._isAdmin = false;
          const expiresIn = await this.anonymousLogin();
          this.scheduleRefresh(expiresIn);
        }
      } else {
        const expiresIn = await this.anonymousLogin();
        this.scheduleRefresh(expiresIn);
      }
    } catch (error) {
      this.isAuthenticated = false;
      this._isAdmin = false;
      console.error('[Auth] Token refresh failed:', error);
      throw error;
    }
  }

  private scheduleRefresh(expiresInSeconds: number): void {
    if (this.refreshTimer !== null) {
      clearTimeout(this.refreshTimer);
    }
    const expiresInMs = Math.max(expiresInSeconds, 0) * 1000;
    const refreshInMs = Math.max(expiresInMs - REFRESH_BUFFER_MS, 5_000);
    this.refreshTimer = window.setTimeout(() => {
      this.refresh().catch((error) => {
        console.error('[Auth] Scheduled refresh failed:', error);
      });
    }, refreshInMs);
  }
}

function secondsUntil(isoTimestamp: string): number {
  const target = Date.parse(isoTimestamp);
  if (Number.isNaN(target)) return FALLBACK_LIFETIME_SECONDS;
  return Math.max(0, Math.floor((target - Date.now()) / 1000));
}

let authServiceInstance: AuthService | null = null;

export function getAuthService(): AuthService {
  if (!authServiceInstance) {
    authServiceInstance = new AuthService();
  }
  return authServiceInstance;
}
