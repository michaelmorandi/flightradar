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
 *
 * Admin Authentication:
 * - Admin users can login via adminLogin()
 * - Admin state is restored from cookie on page reload
 * - Admin logout falls back to anonymous auth
 */

import axios, { type AxiosInstance } from 'axios';
import { config } from '@/config';

/** Refresh token 2 minutes before expiry */
const REFRESH_BUFFER_MS = 2 * 60 * 1000;

/** Token lifetime in seconds (must match backend) */
const TOKEN_LIFETIME_SECONDS = 900; // 15 minutes

/** Anonymous user email */
const ANONYMOUS_EMAIL = 'anonymous@system.local';

/** Admin user email */
const ADMIN_EMAIL = 'admin@system.local';

interface UserInfo {
  email: string;
  role: string;
  is_admin: boolean;
}

export class AuthService {
  private axios: AxiosInstance;
  private apiUrl: string;
  private clientSecret: string;
  private refreshTimer: number | null = null;
  private isAuthenticated = false;
  private _isAdmin = false;
  private adminPassword: string | null = null;

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
   * Check if current user is admin.
   */
  public isAdmin(): boolean {
    return this._isAdmin;
  }

  /**
   * Check current session status by calling /auth/me.
   * Returns user info if authenticated, null otherwise.
   */
  private async checkSession(): Promise<UserInfo | null> {
    try {
      const response = await this.axios.get<UserInfo>(`${this.apiUrl}/auth/me`);
      if (response.status === 200) {
        return response.data;
      }
      return null;
    } catch {
      return null;
    }
  }

  /**
   * Initialize authentication.
   *
   * First checks if there's an existing valid session (e.g., after page reload).
   * If the session is admin, restores admin state.
   * Otherwise, authenticates as anonymous.
   *
   * @throws Error if authentication fails
   */
  public async initialize(): Promise<void> {
    if (!this.apiUrl) {
      throw new Error('Flight API URL not configured');
    }

    console.log('[Auth] Initializing authentication...');

    // First, check if we have an existing valid session
    const existingSession = await this.checkSession();

    if (existingSession) {
      console.log(`[Auth] Existing session found: ${existingSession.role}`);
      this.isAuthenticated = true;
      this._isAdmin = existingSession.is_admin;

      if (this._isAdmin) {
        console.log('[Auth] Restored admin session');
      }

      // Schedule token refresh
      this.scheduleRefresh(TOKEN_LIFETIME_SECONDS);
      return;
    }

    // No existing session, authenticate as anonymous
    if (!this.clientSecret) {
      throw new Error('Client secret not configured');
    }

    try {
      await this.login(ANONYMOUS_EMAIL, this.clientSecret);

      this.isAuthenticated = true;
      this._isAdmin = false;
      console.log('[Auth] Authentication successful');

      // Schedule token refresh
      this.scheduleRefresh(TOKEN_LIFETIME_SECONDS);
    } catch (error) {
      this.isAuthenticated = false;
      this._isAdmin = false;
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
   * Login as admin user using the dedicated admin login endpoint.
   * This endpoint returns 401 on failure (not 400 like the standard endpoint).
   *
   * @param password Admin password
   * @throws Error if login fails
   */
  public async adminLogin(password: string): Promise<void> {
    console.log('[Auth] Logging in as admin...');

    const formData = new URLSearchParams();
    formData.append('username', ADMIN_EMAIL);
    formData.append('password', password);

    try {
      const response = await this.axios.post(`${this.apiUrl}/admin/login`, formData, {
        headers: {
          'Content-Type': 'application/x-www-form-urlencoded',
        },
      });

      if (response.status !== 200) {
        throw new Error('Login failed');
      }

      this._isAdmin = true;
      this.adminPassword = password;
      this.isAuthenticated = true;
      console.log('[Auth] Admin login successful');

      // Schedule token refresh for admin
      this.scheduleRefresh(TOKEN_LIFETIME_SECONDS);
    } catch (error) {
      console.error('[Auth] Admin login failed:', error);
      throw new Error('Invalid admin credentials');
    }
  }

  /**
   * Logout admin and fall back to anonymous auth.
   */
  public async adminLogout(): Promise<void> {
    console.log('[Auth] Admin logout, falling back to anonymous...');

    if (this.refreshTimer !== null) {
      clearTimeout(this.refreshTimer);
      this.refreshTimer = null;
    }

    this._isAdmin = false;
    this.adminPassword = null;

    try {
      // Log out first
      await this.axios.post(`${this.apiUrl}/auth/jwt/logout`);
    } catch (error) {
      console.error('[Auth] Logout request failed:', error);
    }

    // Re-authenticate as anonymous
    try {
      await this.login(ANONYMOUS_EMAIL, this.clientSecret);
      this.isAuthenticated = true;
      console.log('[Auth] Fallback to anonymous auth successful');
      this.scheduleRefresh(TOKEN_LIFETIME_SECONDS);
    } catch (error) {
      this.isAuthenticated = false;
      console.error('[Auth] Fallback to anonymous auth failed:', error);
    }
  }

  /**
   * Refresh the authentication token.
   *
   * For admin sessions without stored password, checks if session is still valid.
   * Otherwise re-logins with stored credentials.
   */
  public async refresh(): Promise<void> {
    if (!this.apiUrl) {
      throw new Error('Authentication not properly configured');
    }

    console.log('[Auth] Refreshing token...');

    try {
      if (this._isAdmin && this.adminPassword) {
        // Refresh as admin with stored password using admin endpoint
        const formData = new URLSearchParams();
        formData.append('username', ADMIN_EMAIL);
        formData.append('password', this.adminPassword);
        await this.axios.post(`${this.apiUrl}/admin/login`, formData, {
          headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        });
        console.log('[Auth] Admin token refreshed successfully');
      } else if (this._isAdmin) {
        // Admin session restored from cookie - check if still valid
        const session = await this.checkSession();
        if (session?.is_admin) {
          console.log('[Auth] Admin session still valid');
        } else {
          // Session expired or no longer admin, fall back to anonymous
          console.log('[Auth] Admin session expired, falling back to anonymous');
          this._isAdmin = false;
          await this.login(ANONYMOUS_EMAIL, this.clientSecret);
        }
      } else {
        // Refresh as anonymous
        if (!this.clientSecret) {
          throw new Error('Client secret not configured');
        }
        await this.login(ANONYMOUS_EMAIL, this.clientSecret);
        console.log('[Auth] Token refreshed successfully');
      }
      this.scheduleRefresh(TOKEN_LIFETIME_SECONDS);
    } catch (error) {
      this.isAuthenticated = false;
      this._isAdmin = false;
      this.adminPassword = null;
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
    this._isAdmin = false;
    this.adminPassword = null;
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
