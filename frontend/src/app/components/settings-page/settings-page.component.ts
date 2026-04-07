import { Component, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { CookieManagementService, SessionStatus, CookieInfo } from '../../services/cookie-management.service';

@Component({
  selector: 'app-settings-page',
  standalone: true,
  imports: [CommonModule, FormsModule],
  templateUrl: './settings-page.component.html',
  styleUrls: ['./settings-page.component.css']
})
export class SettingsPageComponent {
  // Tab state
  activeTab = signal<'session' | 'cookies' | 'config'>('session');
  
  // Cookie input
  manualCookieInput = signal('');
  showManualInput = signal(false);
  
  // Cookie list
  cookieList = signal<CookieInfo[]>([]);
  showCookieList = signal(false);
  
  // Messages
  successMessage = signal<string | null>(null);
  errorMessage = signal<string | null>(null);
  
  // Loading states
  isExtracting = signal(false);
  isSubmitting = signal(false);
  isTesting = signal(false);

  constructor(public cookieService: CookieManagementService) {}

  /**
   * Extract cookies from browser
   */
  extractFromBrowser(): void {
    this.isExtracting.set(true);
    this.clearMessages();
    
    this.cookieService.extractFromBrowser().subscribe({
      next: (result) => {
        this.isExtracting.set(false);
        if (result.success) {
          this.successMessage.set(
            `✅ Успешно извлечено ${result.cookies_count} cookies из ${result.browser || 'браузера'}`
          );
        } else {
          this.errorMessage.set(result.message);
        }
      },
      error: (err) => {
        this.isExtracting.set(false);
        this.errorMessage.set(
          `❌ Ошибка извлечения: ${err.error?.message || err.message || 'Неизвестная ошибка'}`
        );
      }
    });
  }

  /**
   * Submit manual cookies
   */
  submitManualCookies(): void {
    const cookieString = this.manualCookieInput().trim();
    
    if (!cookieString) {
      this.errorMessage.set('Введите cookies');
      return;
    }
    
    this.isSubmitting.set(true);
    this.clearMessages();
    
    this.cookieService.submitManualCookies(cookieString).subscribe({
      next: (result) => {
        this.isSubmitting.set(false);
        if (result.success) {
          this.successMessage.set(
            `✅ Успешно добавлено ${result.cookies_count} cookies`
          );
          this.manualCookieInput.set('');
          this.showManualInput.set(false);
        } else {
          this.errorMessage.set(result.message);
        }
      },
      error: (err) => {
        this.isSubmitting.set(false);
        this.errorMessage.set(
          `❌ Ошибка: ${err.error?.message || err.message || 'Неизвестная ошибка'}`
        );
      }
    });
  }

  /**
   * Refresh session
   */
  refreshSession(): void {
    this.clearMessages();
    this.cookieService.refreshSession().subscribe({
      next: (result) => {
        if (result.success) {
          this.successMessage.set('✅ Сессия обновлена');
        }
      },
      error: (err) => {
        this.errorMessage.set(`❌ Ошибка обновления: ${err.message}`);
      }
    });
  }

  /**
   * Test session validity
   */
  testSession(): void {
    this.isTesting.set(true);
    this.clearMessages();
    
    this.cookieService.testSession().subscribe({
      next: (result) => {
        this.isTesting.set(false);
        if (result.valid) {
          this.successMessage.set(`✅ ${result.message}`);
        } else {
          this.errorMessage.set(`⚠️ ${result.message}`);
        }
      },
      error: (err) => {
        this.isTesting.set(false);
        this.errorMessage.set(`❌ Ошибка теста: ${err.message}`);
      }
    });
  }

  /**
   * Load cookie list
   */
  loadCookieList(): void {
    this.cookieService.getCookieList().subscribe({
      next: (cookies) => {
        this.cookieList.set(cookies);
        this.showCookieList.set(true);
      },
      error: (err) => {
        this.errorMessage.set(`❌ Ошибка загрузки: ${err.message}`);
      }
    });
  }

  /**
   * Clear session
   */
  clearSession(): void {
    if (!confirm('Вы уверены? Это удалит текущую сессию.')) {
      return;
    }
    
    this.clearMessages();
    
    this.cookieService.clearSession().subscribe({
      next: (result) => {
        if (result.success) {
          this.successMessage.set('✅ Сессия очищена');
          this.cookieList.set([]);
        }
      },
      error: (err) => {
        this.errorMessage.set(`❌ Ошибка: ${err.message}`);
      }
    });
  }

  /**
   * Toggle manual input
   */
  toggleManualInput(): void {
    this.showManualInput.set(!this.showManualInput());
    this.clearMessages();
  }

  /**
   * Copy example to clipboard
   */
  copyExample(): void {
    const example = 'session_id=abc123; auth_token=xyz789; token=def456';
    navigator.clipboard.writeText(example).then(() => {
      this.successMessage.set('✅ Пример скопирован в буфер обмена');
      setTimeout(() => this.successMessage.set(null), 2000);
    });
  }

  /**
   * Clear messages
   */
  private clearMessages(): void {
    this.successMessage.set(null);
    this.errorMessage.set(null);
  }

  /**
   * Get status badge class
   */
  getStatusBadgeClass(status: SessionStatus | null): string {
    const color = this.cookieService.getStatusColor(status);
    return `badge-${color}`;
  }

  /**
   * Format expiry time
   */
  formatExpiry(minutes?: number): string {
    return this.cookieService.formatTimeUntilExpiry(minutes);
  }
}
