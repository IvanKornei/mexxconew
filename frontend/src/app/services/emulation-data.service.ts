import { Injectable, signal } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable, interval } from 'rxjs';
import { switchMap, catchError, of } from 'rxjs';

export interface EmulationStatus {
  trading_mode: string;
  session_valid: boolean;
  session_created_at: string | null;
  session_expires_at: string | null;
  background_actions_count: number;
  latency_p50_ms: number;
  latency_p95_ms: number;
  latency_p99_ms: number;
  last_activity: string | null;
}

@Injectable({
  providedIn: 'root'
})
export class EmulationDataService {
  private apiUrl = 'http://localhost:3001/api/emulation/status';
  
  // Signals for reactive state
  emulationStatus = signal<EmulationStatus | null>(null);
  isLoading = signal<boolean>(false);
  error = signal<string | null>(null);

  constructor(private http: HttpClient) {
    this.startPolling();
  }

  /**
   * Start polling emulation status every 5 seconds
   */
  private startPolling(): void {
    interval(5000)
      .pipe(
        switchMap(() => this.fetchEmulationStatus())
      )
      .subscribe({
        next: (status) => {
          this.emulationStatus.set(status);
          this.error.set(null);
        },
        error: (err) => {
          this.error.set('Failed to fetch emulation status');
          console.error('Emulation status error:', err);
        }
      });
  }

  /**
   * Fetch emulation status from backend
   */
  private fetchEmulationStatus(): Observable<EmulationStatus> {
    this.isLoading.set(true);
    
    return this.http.get<EmulationStatus>(this.apiUrl).pipe(
      catchError((error) => {
        console.error('Error fetching emulation status:', error);
        this.isLoading.set(false);
        return of({
          trading_mode: 'Unknown',
          session_valid: false,
          session_created_at: null,
          session_expires_at: null,
          background_actions_count: 0,
          latency_p50_ms: 0,
          latency_p95_ms: 0,
          latency_p99_ms: 0,
          last_activity: null
        });
      })
    );
  }

  /**
   * Manually refresh emulation status
   */
  refresh(): void {
    this.fetchEmulationStatus().subscribe({
      next: (status) => {
        this.emulationStatus.set(status);
        this.isLoading.set(false);
      }
    });
  }
}
