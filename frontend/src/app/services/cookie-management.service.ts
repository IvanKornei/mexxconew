import { Injectable, signal } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable, interval } from 'rxjs';
import { map, catchError } from 'rxjs/operators';

export interface CookieInfo {
  name: string;
  domain: string;
  value_preview: string;
  secure: boolean;
  http_only: boolean;
  expires?: string;
}

export interface SessionStatus {
  is_valid: boolean;
  cookies_count: number;
  expires_at?: string;
  time_until_expiry_minutes?: number;
  last_refresh?: string;
  source: 'browser' | 'manual' | 'automation' | 'cached';
}

export interface CookieExtractionResult {
  success: boolean;
  cookies_count: number;
  browser?: string;
  message: string;
}

@Injectable({
  providedIn: 'root'
})
export class CookieManagementService {
  private apiUrl = 'http://localhost:3001/api';
  
  // Signals for reactive state
  sessionStatus = signal<SessionStatus | null>(null);
  isRefreshing = signal(false);
  lastError = signal<string | null>(null);

  constructor(private http: HttpClient) {
    // Auto-refresh session status every 30 seconds
    interval(30000).subscribe(() => {
      this.refreshSessionStatus();
    });
    
    // Initial load
    this.refreshSessionStatus();
  }

  /**
   * Get current session status
   */
  getSessionStatus(): Observable<SessionStatus> {
    return this.http.get<SessionStatus>(`${this.apiUrl}/session/status`).pipe(
      map(status => {
        this.sessionStatus.set(status);
        this.lastError.set(null);
        return status;
      }),
      catchError(error => {
        this.lastError.set(error.message || 'Failed to get session status');
        throw error;
      })
    );
  }

  /**
   * Extract cookies from browser automatically
   */
  extractFromBrowser(): Observable<CookieExtractionResult> {
    this.isRefreshing.set(true);
    
    return this.http.post<CookieExtractionResult>(
      `${this.apiUrl}/cookies/extract-browser`,
      {}
    ).pipe(
      map(result => {
        this.isRefreshing.set(false);
        if (result.success) {
          this.refreshSessionStatus();
        }
        return result;
      }),
      catchError(error => {
        this.isRefreshing.set(false);
        this.lastError.set(error.message || 'Failed to extract cookies');
        throw error;
      })
    );
  }

  /**
   * Submit manual cookies
   */
  submitManualCookies(cookieString: string): Observable<CookieExtractionResult> {
    this.isRefreshing.set(true);
    
    return this.http.post<CookieExtractionResult>(
      `${this.apiUrl}/cookies/manual`,
      { cookie_string: cookieString }
    ).pipe(
      map(result => {
        this.isRefreshing.set(false);
        if (result.success) {
          this.refreshSessionStatus();
        }
        return result;
      }),
      catchError(error => {
        this.isRefreshing.set(false);
        this.lastError.set(error.message || 'Failed to submit cookies');
        throw error;
      })
    );
  }

  /**
   * Refresh session (re-extract cookies)
   */
  refreshSession(): Observable<CookieExtractionResult> {
    return this.extractFromBrowser();
  }

  /**
   * Get list of cookies (for display)
   */
  getCookieList(): Observable<CookieInfo[]> {
    return this.http.get<CookieInfo[]>(`${this.apiUrl}/cookies/list`);
  }

  /**
   * Clear current session
   */
  clearSession(): Observable<{ success: boolean }> {
    return this.http.delete<{ success: boolean }>(
      `${this.apiUrl}/session/clear`
    ).pipe(
      map(result => {
        if (result.success) {
          this.sessionStatus.set(null);
        }
        return result;
      })
    );
  }

  /**
   * Test session validity
   */
  testSession(): Observable<{ valid: boolean; message: string }> {
    return this.http.post<{ valid: boolean; message: string }>(
      `${this.apiUrl}/session/test`,
      {}
    );
  }

  /**
   * Refresh session status (internal)
   */
  private refreshSessionStatus(): void {
    this.getSessionStatus().subscribe({
      error: (err) => console.error('Failed to refresh session status:', err)
    });
  }

  /**
   * Format time until expiry
   */
  formatTimeUntilExpiry(minutes?: number): string {
    if (!minutes) return 'Unknown';
    
    if (minutes < 60) {
      return `${Math.floor(minutes)} минут`;
    } else if (minutes < 1440) {
      const hours = Math.floor(minutes / 60);
      return `${hours} ${hours === 1 ? 'час' : 'часов'}`;
    } else {
      const days = Math.floor(minutes / 1440);
      return `${days} ${days === 1 ? 'день' : 'дней'}`;
    }
  }

  /**
   * Get status color for UI
   */
  getStatusColor(status: SessionStatus | null): string {
    if (!status || !status.is_valid) return 'red';
    
    const minutes = status.time_until_expiry_minutes || 0;
    if (minutes < 30) return 'red';
    if (minutes < 120) return 'yellow';
    return 'green';
  }
}
