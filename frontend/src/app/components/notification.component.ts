import { Component, Input, Signal } from '@angular/core';
import { CommonModule } from '@angular/common';

export interface Notification {
  type: 'error' | 'warning' | 'info' | 'success';
  message: string;
  timestamp: number;
}

@Component({
  selector: 'app-notification',
  standalone: true,
  imports: [CommonModule],
  template: `
    <div *ngIf="notification()" 
         [class]="getNotificationClass()"
         class="fixed top-4 right-4 max-w-md p-4 rounded-lg shadow-lg z-50 animate-slide-in">
      <div class="flex items-start">
        <div class="flex-shrink-0">
          <svg *ngIf="notification()?.type === 'error'" class="h-6 w-6 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <svg *ngIf="notification()?.type === 'warning'" class="h-6 w-6 text-yellow-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
          <svg *ngIf="notification()?.type === 'success'" class="h-6 w-6 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <svg *ngIf="notification()?.type === 'info'" class="h-6 w-6 text-blue-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        </div>
        <div class="ml-3 flex-1">
          <p class="text-sm font-medium text-white">
            {{ notification()?.message }}
          </p>
        </div>
      </div>
    </div>
  `,
  styles: [`
    @keyframes slide-in {
      from {
        transform: translateX(100%);
        opacity: 0;
      }
      to {
        transform: translateX(0);
        opacity: 1;
      }
    }
    
    .animate-slide-in {
      animation: slide-in 0.3s ease-out;
    }
  `]
})
export class NotificationComponent {
  @Input({ required: true }) notification!: Signal<Notification | null>;
  
  getNotificationClass(): string {
    const type = this.notification()?.type;
    const baseClass = 'border-l-4 ';
    
    switch (type) {
      case 'error':
        return baseClass + 'bg-red-900 border-red-400';
      case 'warning':
        return baseClass + 'bg-yellow-900 border-yellow-400';
      case 'success':
        return baseClass + 'bg-emerald-900 border-emerald-400';
      case 'info':
        return baseClass + 'bg-blue-900 border-blue-400';
      default:
        return baseClass + 'bg-slate-800 border-slate-400';
    }
  }
}
