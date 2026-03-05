import { Component, Output, EventEmitter } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';

export interface TradingSettings {
  exchangeBalance: number;     // Баланс на бирже ($)
  positionSizePercent: number; // Процент на ордер (1-100)
  leverage: number;            // Плечо
  mode: 'conservative' | 'balanced' | 'aggressive';
  maxPositions: number;
  // Параметры предиктивной стратегии
  momentumThreshold: number;
  quickExitTimeout: number;
  takeProfitPercent: number;
  stopLossPercent: number;
  // Веса сигналов (0-100%)
  momentumWeight: number;
  lagWeight: number;
  priceDiffWeight: number;
}

export interface StrategySettings {
  momentum_weight: number;
  lag_weight: number;
  price_diff_weight: number;
  momentum_threshold: number;
  quick_exit_timeout_ms: number;
}

export interface CapitalSettings {
  exchange_balance: number;
  position_size_percent: number;
  leverage: number;
  max_positions: number;
}

@Component({
  selector: 'app-trading-settings',
  standalone: true,
  imports: [CommonModule, FormsModule],
  template: `
    <div class="card">
      <h3 class="text-gray-400 text-sm uppercase tracking-wide mb-4">
        Trading Settings (Simulation)
      </h3>
      
      <!-- Apply Button -->
      <div *ngIf="hasChanges" class="mb-4 p-3 bg-emerald-900 bg-opacity-20 border border-emerald-700 rounded">
        <div class="flex items-center justify-between">
          <div>
            <div class="text-emerald-400 font-bold text-sm">Settings Changed</div>
            <div class="text-emerald-300 text-xs">Click Apply to update strategy</div>
          </div>
          <button 
            (click)="applySettings()"
            class="px-4 py-2 bg-emerald-600 hover:bg-emerald-500 text-white rounded font-medium transition">
            Apply Settings
          </button>
        </div>
      </div>
      
      <div class="grid grid-cols-3 gap-4 mb-4">
        <div>
          <label class="text-gray-500 text-xs block mb-1">Exchange Balance ($)</label>
          <input 
            type="number" 
            [(ngModel)]="settings.exchangeBalance"
            (change)="onSettingsChange()"
            class="w-full bg-slate-900 text-white px-3 py-2 rounded border border-gray-700 focus:border-emerald-500 focus:outline-none"
            min="1"
            step="1">
        </div>
        <div>
          <label class="text-gray-500 text-xs block mb-1">Position Size (%)</label>
          <input 
            type="number" 
            [(ngModel)]="settings.positionSizePercent"
            (change)="onSettingsChange()"
            class="w-full bg-slate-900 text-white px-3 py-2 rounded border border-gray-700 focus:border-emerald-500 focus:outline-none"
            min="1"
            max="100"
            step="1">
        </div>
        <div>
          <label class="text-gray-500 text-xs block mb-1">Leverage (x)</label>
          <input 
            type="number" 
            [(ngModel)]="settings.leverage"
            (change)="onSettingsChange()"
            class="w-full bg-slate-900 text-white px-3 py-2 rounded border border-gray-700 focus:border-emerald-500 focus:outline-none"
            min="1"
            max="200"
            step="1">
        </div>
      </div>
      
      <div class="mb-4 p-3 bg-slate-900 rounded border border-emerald-900">
        <div class="text-xs text-gray-500">Order Size (per trade)</div>
        <div class="text-2xl font-bold text-emerald-400">
          \${{ getOrderSize() | number:'1.0-0' }}
        </div>
        <div class="text-xs text-gray-600 mt-1">
          \${{ settings.exchangeBalance }} × {{ settings.positionSizePercent }}% × {{ settings.leverage }}x leverage
        </div>
      </div>
      
      <div class="mb-4">
        <label class="text-gray-500 text-xs block mb-2">Strategy Mode</label>
        <div class="grid grid-cols-3 gap-2">
          <button 
            (click)="setMode('conservative')"
            [class]="settings.mode === 'conservative' ? 'bg-blue-900 text-blue-400 border-blue-500' : 'bg-slate-900 text-gray-400 border-gray-700'"
            class="px-3 py-2 rounded border text-sm font-medium hover:border-blue-500 transition">
            Safe
          </button>
          <button 
            (click)="setMode('balanced')"
            [class]="settings.mode === 'balanced' ? 'bg-emerald-900 text-emerald-400 border-emerald-500' : 'bg-slate-900 text-gray-400 border-gray-700'"
            class="px-3 py-2 rounded border text-sm font-medium hover:border-emerald-500 transition">
            Balanced
          </button>
          <button 
            (click)="setMode('aggressive')"
            [class]="settings.mode === 'aggressive' ? 'bg-red-900 text-red-400 border-red-500' : 'bg-slate-900 text-gray-400 border-gray-700'"
            class="px-3 py-2 rounded border text-sm font-medium hover:border-red-500 transition">
            Aggressive
          </button>
        </div>
      </div>
      
      <details class="mb-4">
        <summary class="text-gray-400 text-sm cursor-pointer hover:text-white">
          Advanced Parameters (Predictive Strategy)
        </summary>
        <div class="mt-3 space-y-3">
          <div>
            <label class="text-gray-500 text-xs block mb-1">
              Max Positions: {{ settings.maxPositions }}
            </label>
            <input 
              type="range" 
              [(ngModel)]="settings.maxPositions"
              (change)="onSettingsChange()"
              class="w-full"
              min="1"
              max="5"
              step="1">
          </div>
          <div>
            <label class="text-gray-500 text-xs block mb-1">
              Momentum Threshold: {{ settings.momentumThreshold }}%
              <span class="text-gray-600">(impulse per 100ms)</span>
            </label>
            <input 
              type="range" 
              [(ngModel)]="settings.momentumThreshold"
              (change)="onSettingsChange()"
              class="w-full"
              min="0.01"
              max="0.10"
              step="0.01">
          </div>
          <div>
            <label class="text-gray-500 text-xs block mb-1">
              Quick Exit Timeout: {{ settings.quickExitTimeout }}ms
              <span class="text-gray-600">(max time in position)</span>
            </label>
            <input 
              type="range" 
              [(ngModel)]="settings.quickExitTimeout"
              (change)="onSettingsChange()"
              class="w-full"
              min="1000"
              max="5000"
              step="500">
          </div>
          <div>
            <label class="text-gray-500 text-xs block mb-1">
              Take Profit: {{ settings.takeProfitPercent }}%
              <span class="text-gray-600">(exit target)</span>
            </label>
            <input 
              type="range" 
              [(ngModel)]="settings.takeProfitPercent"
              (change)="onSettingsChange()"
              class="w-full"
              min="0.02"
              max="0.10"
              step="0.01">
          </div>
          <div>
            <label class="text-gray-500 text-xs block mb-1">
              Stop Loss: {{ settings.stopLossPercent }}%
              <span class="text-gray-600">(max loss)</span>
            </label>
            <input 
              type="range" 
              [(ngModel)]="settings.stopLossPercent"
              (change)="onSettingsChange()"
              class="w-full"
              min="0.01"
              max="0.05"
              step="0.005">
          </div>
          
          <!-- Signal Weights -->
          <div class="mt-4 pt-4 border-t border-gray-700">
            <div class="text-gray-400 text-xs mb-2">Signal Weights (must sum to 100%)</div>
            <div>
              <label class="text-gray-500 text-xs block mb-1">
                Momentum: {{ settings.momentumWeight }}%
                <span class="text-gray-600">(price impulse)</span>
              </label>
              <input 
                type="range" 
                [(ngModel)]="settings.momentumWeight"
                (change)="onSettingsChange()"
                class="w-full"
                min="0"
                max="100"
                step="5">
            </div>
            <div>
              <label class="text-gray-500 text-xs block mb-1">
                Lag Statistics: {{ settings.lagWeight }}%
                <span class="text-gray-600">(historical lag)</span>
              </label>
              <input 
                type="range" 
                [(ngModel)]="settings.lagWeight"
                (change)="onSettingsChange()"
                class="w-full"
                min="0"
                max="100"
                step="5">
            </div>
            <div>
              <label class="text-gray-500 text-xs block mb-1">
                Price Difference: {{ settings.priceDiffWeight }}%
                <span class="text-gray-600">(current spread)</span>
              </label>
              <input 
                type="range" 
                [(ngModel)]="settings.priceDiffWeight"
                (change)="onSettingsChange()"
                class="w-full"
                min="0"
                max="100"
                step="5">
            </div>
            <div class="text-xs mt-2" [class]="getTotalWeight() === 100 ? 'text-emerald-400' : 'text-red-400'">
              Total: {{ getTotalWeight() }}% {{ getTotalWeight() === 100 ? '✓' : '(should be 100%)' }}
            </div>
          </div>
        </div>
      </details>
      
      <div class="p-3 bg-yellow-900 bg-opacity-20 border border-yellow-700 rounded text-xs">
        <div class="text-yellow-400 font-bold mb-1">Warning: High Leverage Risk</div>
        <div class="text-yellow-300">
          {{ settings.leverage }}x leverage multiplies both profit and loss by {{ settings.leverage }}x.
          Price move of {{ getLiquidationPercent() }}% against you = liquidation.
        </div>
      </div>
    </div>
  `,
  styles: []
})
export class TradingSettingsComponent {
  @Output() settingsChanged = new EventEmitter<TradingSettings>();
  @Output() applyClicked = new EventEmitter<{strategy: StrategySettings, capital: CapitalSettings}>();
  
  hasChanges = false;
  
  settings: TradingSettings = {
    exchangeBalance: 100,      // $100 на бирже
    positionSizePercent: 10,   // 10% на ордер
    leverage: 200,             // Плечо 200x
    mode: 'balanced',
    maxPositions: 2,
    momentumThreshold: 0.03,
    quickExitTimeout: 2000,
    takeProfitPercent: 0.04,
    stopLossPercent: 0.02,
    momentumWeight: 60,
    lagWeight: 30,
    priceDiffWeight: 10,
  };
  
  ngOnInit() {
    this.onSettingsChange();
  }
  
  applySettings() {
    const strategySettings: StrategySettings = {
      momentum_weight: this.settings.momentumWeight / 100,
      lag_weight: this.settings.lagWeight / 100,
      price_diff_weight: this.settings.priceDiffWeight / 100,
      momentum_threshold: this.settings.momentumThreshold,
      quick_exit_timeout_ms: this.settings.quickExitTimeout,
    };
    
    const capitalSettings: CapitalSettings = {
      exchange_balance: this.settings.exchangeBalance,
      position_size_percent: this.settings.positionSizePercent,
      leverage: this.settings.leverage,
      max_positions: this.settings.maxPositions,
    };
    
    this.applyClicked.emit({strategy: strategySettings, capital: capitalSettings});
    this.hasChanges = false;
  }
  
  setMode(mode: 'conservative' | 'balanced' | 'aggressive') {
    this.settings.mode = mode;
    
    switch (mode) {
      case 'conservative':
        this.settings.maxPositions = 1;
        this.settings.momentumThreshold = 0.05;
        this.settings.quickExitTimeout = 1500;
        this.settings.takeProfitPercent = 0.03;
        this.settings.stopLossPercent = 0.015;
        // Консервативные веса: больше полагаемся на статистику
        this.settings.momentumWeight = 40;
        this.settings.lagWeight = 50;
        this.settings.priceDiffWeight = 10;
        break;
      case 'balanced':
        this.settings.maxPositions = 2;
        this.settings.momentumThreshold = 0.03;
        this.settings.quickExitTimeout = 2000;
        this.settings.takeProfitPercent = 0.04;
        this.settings.stopLossPercent = 0.02;
        // Сбалансированные веса
        this.settings.momentumWeight = 60;
        this.settings.lagWeight = 30;
        this.settings.priceDiffWeight = 10;
        break;
      case 'aggressive':
        this.settings.maxPositions = 3;
        this.settings.momentumThreshold = 0.02;
        this.settings.quickExitTimeout = 3000;
        this.settings.takeProfitPercent = 0.06;
        this.settings.stopLossPercent = 0.03;
        // Агрессивные веса: больше полагаемся на импульс
        this.settings.momentumWeight = 80;
        this.settings.lagWeight = 15;
        this.settings.priceDiffWeight = 5;
        break;
    }
    
    this.onSettingsChange();
  }
  
  getOrderSize(): number {
    return this.settings.exchangeBalance * (this.settings.positionSizePercent / 100) * this.settings.leverage;
  }
  
  getLiquidationPercent(): string {
    return (100 / this.settings.leverage).toFixed(2);
  }
  
  getTotalWeight(): number {
    return this.settings.momentumWeight + this.settings.lagWeight + this.settings.priceDiffWeight;
  }
  
  onSettingsChange() {
    this.hasChanges = true;
    this.settingsChanged.emit(this.settings);
  }
}
