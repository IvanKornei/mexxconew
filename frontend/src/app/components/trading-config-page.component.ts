import { Component, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { HttpClient } from '@angular/common/http';

interface TradingConfig {
  mode: 'DryRun' | 'Paper' | 'Live';
  auto_trading_enabled: boolean;
  min_spread_percent: number;
  position_size_percent: number;
  leverage: number;
  max_positions: number;
}

interface TradingStatus {
  is_running: boolean;
  mode: string;
  auto_trading_enabled: boolean;
  current_position: any;
  stats: any;
}

@Component({
  selector: 'app-trading-config-page',
  standalone: true,
  imports: [CommonModule, FormsModule],
  template: `
    <div class="min-h-screen bg-gray-900 text-white p-6">
      <div class="max-w-4xl mx-auto">
        <h1 class="text-3xl font-bold mb-8">⚙️ Trading Configuration</h1>

        <!-- Status Card -->
        <div class="bg-gray-800 rounded-lg p-6 mb-6 border border-gray-700">
          <h2 class="text-xl font-semibold mb-4">📊 Current Status</h2>
          <div class="grid grid-cols-2 gap-4">
            <div>
              <span class="text-gray-400">Trading Status:</span>
              <span [class]="status().is_running ? 'text-green-400 ml-2' : 'text-red-400 ml-2'">
                {{ status().is_running ? '🟢 Running' : '🔴 Stopped' }}
              </span>
            </div>
            <div>
              <span class="text-gray-400">Mode:</span>
              <span class="text-blue-400 ml-2">{{ status().mode }}</span>
            </div>
            <div>
              <span class="text-gray-400">Auto Trading:</span>
              <span [class]="status().auto_trading_enabled ? 'text-green-400 ml-2' : 'text-yellow-400 ml-2'">
                {{ status().auto_trading_enabled ? '✅ Enabled' : '⚠️ Disabled' }}
              </span>
            </div>
          </div>
        </div>

        <!-- Configuration Form -->
        <div class="bg-gray-800 rounded-lg p-6 mb-6 border border-gray-700">
          <h2 class="text-xl font-semibold mb-4">🎛️ Configuration</h2>
          
          <!-- Execution Mode -->
          <div class="mb-6">
            <label class="block text-sm font-medium mb-2">Execution Mode</label>
            <select 
              [(ngModel)]="config().mode"
              class="w-full bg-gray-700 border border-gray-600 rounded px-4 py-2 focus:outline-none focus:border-blue-500">
              <option value="DryRun">🔍 Dry Run (Logging Only)</option>
              <option value="Paper">📝 Paper Trading (Simulation)</option>
              <option value="Live">🚀 Live Trading (Real Money)</option>
            </select>
            <p class="text-sm text-gray-400 mt-1">
              @if (config().mode === 'DryRun') {
                Monitors prices and logs signals without executing trades
              } @else if (config().mode === 'Paper') {
                Simulates trades with real prices, tracks P&L
              } @else {
                ⚠️ REAL MONEY - Places actual orders on exchanges
              }
            </p>
          </div>

          <!-- Auto Trading -->
          <div class="mb-6">
            <label class="flex items-center cursor-pointer">
              <input 
                type="checkbox" 
                [(ngModel)]="config().auto_trading_enabled"
                class="w-5 h-5 text-blue-600 bg-gray-700 border-gray-600 rounded focus:ring-blue-500">
              <span class="ml-3 text-sm font-medium">Enable Auto Trading</span>
            </label>
            <p class="text-sm text-gray-400 mt-1">
              When enabled, system will automatically execute trades based on signals
            </p>
          </div>

          <!-- Strategy Parameters -->
          <div class="grid grid-cols-2 gap-4 mb-6">
            <div>
              <label class="block text-sm font-medium mb-2">Min Spread (%)</label>
              <input 
                type="number" 
                step="0.01"
                [(ngModel)]="config().min_spread_percent"
                class="w-full bg-gray-700 border border-gray-600 rounded px-4 py-2 focus:outline-none focus:border-blue-500">
              <p class="text-xs text-gray-400 mt-1">Minimum spread to open position</p>
            </div>

            <div>
              <label class="block text-sm font-medium mb-2">Position Size (%)</label>
              <input 
                type="number" 
                step="1"
                [(ngModel)]="config().position_size_percent"
                class="w-full bg-gray-700 border border-gray-600 rounded px-4 py-2 focus:outline-none focus:border-blue-500">
              <p class="text-xs text-gray-400 mt-1">% of capital per trade</p>
            </div>

            <div>
              <label class="block text-sm font-medium mb-2">Leverage</label>
              <input 
                type="number" 
                [(ngModel)]="config().leverage"
                class="w-full bg-gray-700 border border-gray-600 rounded px-4 py-2 focus:outline-none focus:border-blue-500">
              <p class="text-xs text-gray-400 mt-1">Trading leverage (1-200x)</p>
            </div>

            <div>
              <label class="block text-sm font-medium mb-2">Max Positions</label>
              <input 
                type="number" 
                [(ngModel)]="config().max_positions"
                class="w-full bg-gray-700 border border-gray-600 rounded px-4 py-2 focus:outline-none focus:border-blue-500">
              <p class="text-xs text-gray-400 mt-1">Maximum concurrent positions</p>
            </div>
          </div>

          <!-- Action Buttons -->
          <div class="flex gap-4">
            <button 
              (click)="saveConfig()"
              [disabled]="saving()"
              class="flex-1 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-600 text-white font-semibold py-3 px-6 rounded transition">
              {{ saving() ? '⏳ Saving...' : '💾 Save Configuration' }}
            </button>
            
            @if (!status().is_running) {
              <button 
                (click)="startTrading()"
                class="flex-1 bg-green-600 hover:bg-green-700 text-white font-semibold py-3 px-6 rounded transition">
                🚀 Start Trading
              </button>
            } @else {
              <button 
                (click)="stopTrading()"
                class="flex-1 bg-red-600 hover:bg-red-700 text-white font-semibold py-3 px-6 rounded transition">
                ⏹️ Stop Trading
              </button>
            }
          </div>
        </div>

        <!-- Warning for Live Mode -->
        @if (config().mode === 'Live') {
          <div class="bg-red-900/30 border border-red-500 rounded-lg p-4 mb-6">
            <h3 class="text-red-400 font-semibold mb-2">⚠️ LIVE TRADING WARNING</h3>
            <ul class="text-sm text-red-300 space-y-1">
              <li>• Real money will be used for trading</li>
              <li>• Ensure you have tested thoroughly in Paper mode</li>
              <li>• Start with small capital to verify system behavior</li>
              <li>• Monitor positions actively during initial runs</li>
              <li>• Have stop-loss and risk management in place</li>
            </ul>
          </div>
        }

        <!-- Success/Error Messages -->
        @if (message()) {
          <div [class]="messageType() === 'success' ? 'bg-green-900/30 border-green-500' : 'bg-red-900/30 border-red-500'"
               class="border rounded-lg p-4">
            <p [class]="messageType() === 'success' ? 'text-green-400' : 'text-red-400'">
              {{ message() }}
            </p>
          </div>
        }
      </div>
    </div>
  `,
  styles: [`
    input[type="number"]::-webkit-inner-spin-button,
    input[type="number"]::-webkit-outer-spin-button {
      opacity: 1;
    }
  `]
})
export class TradingConfigPageComponent {
  private apiUrl = 'http://localhost:3001/api/trading';

  config = signal<TradingConfig>({
    mode: 'DryRun',
    auto_trading_enabled: false,
    min_spread_percent: 0.3,
    position_size_percent: 10,
    leverage: 200,
    max_positions: 2
  });

  status = signal<TradingStatus>({
    is_running: false,
    mode: 'DryRun',
    auto_trading_enabled: false,
    current_position: null,
    stats: null
  });

  saving = signal(false);
  message = signal('');
  messageType = signal<'success' | 'error'>('success');

  constructor(private http: HttpClient) {
    this.loadStatus();
  }

  async loadStatus() {
    try {
      const status = await this.http.get<TradingStatus>(`${this.apiUrl}/status`).toPromise();
      if (status) {
        this.status.set(status);
      }
    } catch (error) {
      console.error('Failed to load status:', error);
    }
  }

  async saveConfig() {
    this.saving.set(true);
    this.message.set('');

    try {
      await this.http.post(`${this.apiUrl}/config`, this.config()).toPromise();
      this.message.set('✅ Configuration saved successfully');
      this.messageType.set('success');
      
      setTimeout(() => this.message.set(''), 3000);
    } catch (error) {
      this.message.set('❌ Failed to save configuration');
      this.messageType.set('error');
      console.error('Save error:', error);
    } finally {
      this.saving.set(false);
    }
  }

  async startTrading() {
    try {
      await this.http.post(`${this.apiUrl}/start`, {}).toPromise();
      this.message.set('✅ Trading started');
      this.messageType.set('success');
      await this.loadStatus();
      
      setTimeout(() => this.message.set(''), 3000);
    } catch (error) {
      this.message.set('❌ Failed to start trading');
      this.messageType.set('error');
      console.error('Start error:', error);
    }
  }

  async stopTrading() {
    try {
      await this.http.post(`${this.apiUrl}/stop`, {}).toPromise();
      this.message.set('✅ Trading stopped');
      this.messageType.set('success');
      await this.loadStatus();
      
      setTimeout(() => this.message.set(''), 3000);
    } catch (error) {
      this.message.set('❌ Failed to stop trading');
      this.messageType.set('error');
      console.error('Stop error:', error);
    }
  }
}
