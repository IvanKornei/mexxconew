import { Component, signal, computed, effect } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { HttpClient } from '@angular/common/http';
import { interval } from 'rxjs';

interface BrowserStatus {
  is_running: boolean;
  is_logged_in: boolean;
  current_url: string;
  session_duration_seconds: number;
  last_activity: string;
  mode: 'visible' | 'headless';
  stealth_enabled: boolean;
  human_behavior_enabled: boolean;
}

interface BrowserConfig {
  mode: 'visible' | 'headless';
  window_width: number;
  window_height: number;
  stealth_mode: boolean;
  human_behavior: boolean;
  typing_delay_min: number;
  typing_delay_max: number;
  click_delay_min: number;
  click_delay_max: number;
  auto_activity_enabled: boolean;
  auto_activity_interval_minutes: number;
  screenshot_on_error: boolean;
  timezone: string;
  language: string;
}

interface TradingAction {
  type: 'market' | 'limit' | 'cancel';
  side?: 'BUY' | 'SELL';
  price?: number;
  quantity?: number;
  order_id?: string;
}

interface Position {
  symbol: string;
  side: string;
  size: number;
  entry_price: number;
  pnl: number;
  leverage: number;
}

@Component({
  selector: 'app-browser-control',
  standalone: true,
  imports: [CommonModule, FormsModule],
  templateUrl: './browser-control.component.html',
  styleUrls: ['./browser-control.component.css']
})
export class BrowserControlComponent {
  private apiUrl = 'http://localhost:3001/api/browser';
  
  // State signals
  browserStatus = signal<BrowserStatus | null>(null);
  browserConfig = signal<BrowserConfig>({
    mode: 'headless',
    window_width: 1920,
    window_height: 1080,
    stealth_mode: true,
    human_behavior: true,
    typing_delay_min: 100,
    typing_delay_max: 300,
    click_delay_min: 200,
    click_delay_max: 500,
    auto_activity_enabled: true,
    auto_activity_interval_minutes: 10,
    screenshot_on_error: true,
    timezone: 'Europe/Moscow',
    language: 'ru-RU',
  });
  
  balance = signal<number>(0);
  positions = signal<Position[]>([]);
  
  // UI state
  activeTab = signal<'control' | 'config' | 'trading' | 'monitoring'>('control');
  isLoading = signal(false);
  statusMessage = signal<{ type: 'success' | 'error' | 'info', text: string } | null>(null);
  
  // Trading form
  tradingForm = signal({
    type: 'market' as 'market' | 'limit',
    side: 'BUY' as 'BUY' | 'SELL',
    price: 50000,
    quantity: 0.001,
  });
  
  // Computed
  isRunning = computed(() => this.browserStatus()?.is_running ?? false);
  isLoggedIn = computed(() => this.browserStatus()?.is_logged_in ?? false);
  sessionDuration = computed(() => {
    const seconds = this.browserStatus()?.session_duration_seconds ?? 0;
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    return `${hours}ч ${minutes}м`;
  });

  constructor(private http: HttpClient) {
    // Auto-refresh status every 5 seconds
    interval(5000).subscribe(() => {
      if (this.isRunning()) {
        this.refreshStatus();
      }
    });
    
    // Initial load
    this.refreshStatus();
    this.loadConfig();
  }

  // ============ Browser Control ============
  
  async startBrowser(): Promise<void> {
    this.isLoading.set(true);
    this.clearMessage();
    
    try {
      const response = await this.http.post<any>(
        `${this.apiUrl}/start`,
        this.browserConfig()
      ).toPromise();
      
      this.showMessage('success', '✅ Браузер запущен успешно');
      await this.refreshStatus();
    } catch (error: any) {
      this.showMessage('error', `❌ Ошибка запуска: ${error.error?.message || error.message}`);
    } finally {
      this.isLoading.set(false);
    }
  }
  
  async stopBrowser(): Promise<void> {
    if (!confirm('Остановить браузер? Все открытые позиции останутся активными.')) {
      return;
    }
    
    this.isLoading.set(true);
    
    try {
      await this.http.post(`${this.apiUrl}/stop`, {}).toPromise();
      this.showMessage('success', '✅ Браузер остановлен');
      this.browserStatus.set(null);
    } catch (error: any) {
      this.showMessage('error', `❌ Ошибка: ${error.message}`);
    } finally {
      this.isLoading.set(false);
    }
  }
  
  async restartBrowser(): Promise<void> {
    this.isLoading.set(true);
    
    try {
      await this.stopBrowser();
      await new Promise(resolve => setTimeout(resolve, 2000));
      await this.startBrowser();
    } finally {
      this.isLoading.set(false);
    }
  }
  
  async takeScreenshot(): Promise<void> {
    this.isLoading.set(true);
    
    try {
      const response = await this.http.post<{ path: string }>(
        `${this.apiUrl}/screenshot`,
        {}
      ).toPromise();
      
      this.showMessage('success', `📸 Скриншот сохранен: ${response?.path}`);
    } catch (error: any) {
      this.showMessage('error', `❌ Ошибка: ${error.message}`);
    } finally {
      this.isLoading.set(false);
    }
  }
  
  async simulateActivity(): Promise<void> {
    this.isLoading.set(true);
    
    try {
      await this.http.post(`${this.apiUrl}/simulate-activity`, {}).toPromise();
      this.showMessage('success', '✅ Активность симулирована');
    } catch (error: any) {
      this.showMessage('error', `❌ Ошибка: ${error.message}`);
    } finally {
      this.isLoading.set(false);
    }
  }

  // ============ Configuration ============
  
  async loadConfig(): Promise<void> {
    try {
      const config = await this.http.get<BrowserConfig>(`${this.apiUrl}/config`).toPromise();
      if (config) {
        this.browserConfig.set(config);
      }
    } catch (error) {
      console.error('Failed to load config:', error);
    }
  }
  
  async saveConfig(): Promise<void> {
    this.isLoading.set(true);
    
    try {
      await this.http.post(`${this.apiUrl}/config`, this.browserConfig()).toPromise();
      this.showMessage('success', '✅ Конфигурация сохранена');
    } catch (error: any) {
      this.showMessage('error', `❌ Ошибка: ${error.message}`);
    } finally {
      this.isLoading.set(false);
    }
  }
  
  resetConfig(): void {
    if (!confirm('Сбросить настройки к значениям по умолчанию?')) {
      return;
    }
    
    this.browserConfig.set({
      mode: 'headless',
      window_width: 1920,
      window_height: 1080,
      stealth_mode: true,
      human_behavior: true,
      typing_delay_min: 100,
      typing_delay_max: 300,
      click_delay_min: 200,
      click_delay_max: 500,
      auto_activity_enabled: true,
      auto_activity_interval_minutes: 10,
      screenshot_on_error: true,
      timezone: 'Europe/Moscow',
      language: 'ru-RU',
    });
    
    this.showMessage('info', 'ℹ️ Настройки сброшены. Нажмите "Сохранить" для применения.');
  }

  // ============ Trading ============
  
  async placeOrder(): Promise<void> {
    const form = this.tradingForm();
    
    if (!confirm(`Разместить ${form.type} ${form.side} ордер на ${form.quantity} BTC?`)) {
      return;
    }
    
    this.isLoading.set(true);
    
    try {
      const action: TradingAction = {
        type: form.type,
        side: form.side,
        quantity: form.quantity,
      };
      
      if (form.type === 'limit') {
        action.price = form.price;
      }
      
      const response = await this.http.post<{ order_id: string }>(
        `${this.apiUrl}/trade`,
        action
      ).toPromise();
      
      this.showMessage('success', `✅ Ордер размещен: ${response?.order_id}`);
      await this.refreshPositions();
    } catch (error: any) {
      this.showMessage('error', `❌ Ошибка: ${error.error?.message || error.message}`);
    } finally {
      this.isLoading.set(false);
    }
  }
  
  async cancelOrder(orderId: string): Promise<void> {
    if (!confirm(`Отменить ордер ${orderId}?`)) {
      return;
    }
    
    this.isLoading.set(true);
    
    try {
      await this.http.post(`${this.apiUrl}/trade`, {
        type: 'cancel',
        order_id: orderId,
      }).toPromise();
      
      this.showMessage('success', '✅ Ордер отменен');
      await this.refreshPositions();
    } catch (error: any) {
      this.showMessage('error', `❌ Ошибка: ${error.message}`);
    } finally {
      this.isLoading.set(false);
    }
  }

  // ============ Monitoring ============
  
  async refreshStatus(): Promise<void> {
    try {
      const status = await this.http.get<BrowserStatus>(`${this.apiUrl}/status`).toPromise();
      if (status) {
        this.browserStatus.set(status);
      }
    } catch (error) {
      // Silently fail for status updates
    }
  }
  
  async refreshBalance(): Promise<void> {
    this.isLoading.set(true);
    
    try {
      const response = await this.http.get<{ balance: number }>(
        `${this.apiUrl}/balance`
      ).toPromise();
      
      if (response) {
        this.balance.set(response.balance);
      }
    } catch (error: any) {
      this.showMessage('error', `❌ Ошибка: ${error.message}`);
    } finally {
      this.isLoading.set(false);
    }
  }
  
  async refreshPositions(): Promise<void> {
    this.isLoading.set(true);
    
    try {
      const positions = await this.http.get<Position[]>(
        `${this.apiUrl}/positions`
      ).toPromise();
      
      if (positions) {
        this.positions.set(positions);
      }
    } catch (error: any) {
      this.showMessage('error', `❌ Ошибка: ${error.message}`);
    } finally {
      this.isLoading.set(false);
    }
  }

  // ============ Utilities ============
  
  private showMessage(type: 'success' | 'error' | 'info', text: string): void {
    this.statusMessage.set({ type, text });
    setTimeout(() => this.statusMessage.set(null), 5000);
  }
  
  private clearMessage(): void {
    this.statusMessage.set(null);
  }
  
  updateConfig<K extends keyof BrowserConfig>(key: K, value: BrowserConfig[K]): void {
    this.browserConfig.update(config => ({ ...config, [key]: value }));
  }
  
  updateTradingForm<K extends keyof typeof this.tradingForm>(
    key: K,
    value: any
  ): void {
    this.tradingForm.update(form => ({ ...form, [key]: value }));
  }
  
  getStatusColor(): string {
    if (!this.isRunning()) return 'gray';
    if (!this.isLoggedIn()) return 'yellow';
    return 'green';
  }
  
  getStatusText(): string {
    if (!this.isRunning()) return 'Остановлен';
    if (!this.isLoggedIn()) return 'Запущен (не залогинен)';
    return 'Активен';
  }
}
