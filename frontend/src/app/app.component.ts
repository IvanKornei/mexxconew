import { Component, computed, OnDestroy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MarketDataService } from './services/market-data.service';
import { PriceCardComponent } from './components/price-card.component';
import { SpreadVisualizerComponent } from './components/spread-visualizer.component';
import { HistoryTableComponent } from './components/history-table.component';
import { PositionsPanelComponent } from './components/positions-panel.component';
import { TradingSettingsComponent, TradingSettings } from './components/trading-settings.component';
import { TradingStatsComponent } from './components/trading-stats.component';

@Component({
  selector: 'app-root',
  standalone: true,
  imports: [
    CommonModule, 
    PriceCardComponent, 
    SpreadVisualizerComponent, 
    HistoryTableComponent, 
    PositionsPanelComponent,
    TradingSettingsComponent,
    TradingStatsComponent
  ],
  template: `
    <div class="min-h-screen bg-slate-900 p-8">
      <div class="max-w-7xl mx-auto">
        <!-- Header -->
        <div class="mb-8">
          <h1 class="text-4xl font-bold text-white mb-2">
            HFT Arbitrage Monitor
          </h1>
          <p class="text-gray-400">
            BTC/USDT Spread Monitoring - Binance Futures vs MEXC Futures
          </p>
        </div>
        
        <!-- Main Grid -->
        <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
          <!-- Binance Price -->
          <app-price-card 
            exchange="Binance Futures"
            [price]="binancePrice">
          </app-price-card>
          
          <!-- MEXC Price -->
          <app-price-card 
            exchange="MEXC Futures"
            [price]="mexcPrice">
          </app-price-card>
          
          <!-- Spread Visualizer -->
          <app-spread-visualizer
            [spread]="spread"
            [latency]="latency"
            [isStale]="isStale">
          </app-spread-visualizer>
        </div>
        
        <!-- Stats Table -->
        <div class="mt-8 card">
          <h3 class="text-gray-400 text-sm uppercase tracking-wide mb-4">
            Market Statistics
          </h3>
          <div class="grid grid-cols-2 md:grid-cols-4 gap-4 font-mono text-sm">
            <div>
              <div class="text-gray-500">Binance</div>
              <div class="text-white">{{ binancePrice() | number:'1.2-2' }}</div>
            </div>
            <div>
              <div class="text-gray-500">MEXC</div>
              <div class="text-white">{{ mexcPrice() | number:'1.2-2' }}</div>
            </div>
            <div>
              <div class="text-gray-500">Price Diff</div>
              <div [class]="priceDiff() > 0 ? 'text-emerald-400' : 'text-red-400'">
                {{ priceDiff() | number:'1.2-2' }} ({{ priceDiffPercent() | number:'1.3-3' }}%)
              </div>
            </div>
            <div>
              <div class="text-gray-500">Status</div>
              <div [class]="isStale() ? 'text-red-400' : 'text-emerald-400'">
                {{ isStale() ? 'STALE' : 'LIVE' }}
              </div>
            </div>
          </div>
        </div>
        
        <!-- Lag Analysis -->
        <div class="mt-6 card">
          <h3 class="text-gray-400 text-sm uppercase tracking-wide mb-4">
            Real-Time Lag Analysis
          </h3>
          <div class="grid grid-cols-1 md:grid-cols-3 gap-4 font-mono text-sm">
            <div>
              <div class="text-gray-500">MEXC Lag (Exchange Time)</div>
              <div [class]="absValue(mexcLag()) > 2000 ? 'text-emerald-400 text-2xl font-bold' : absValue(mexcLag()) > 1000 ? 'text-yellow-400 text-xl' : 'text-white'">
                {{ mexcLag() }}ms
              </div>
              <div class="text-xs mt-1" [class]="mexcLag() > 0 ? 'text-emerald-400' : 'text-gray-400'">
                {{ mexcLag() > 0 ? 'MEXC delayed - OPPORTUNITY!' : 'MEXC synchronized' }}
              </div>
            </div>
            <div>
              <div class="text-gray-500">Network Lag (Our Connection)</div>
              <div class="text-white">
                {{ realLag() }}ms
              </div>
              <div class="text-xs text-gray-600 mt-1">
                Difference in data arrival time
              </div>
            </div>
            <div>
              <div class="text-gray-500">System Latency</div>
              <div [class]="latency() > 100 ? 'text-red-400' : latency() > 50 ? 'text-yellow-400' : 'text-emerald-400'">
                {{ latency() }}ms
              </div>
              <div class="text-xs text-gray-600 mt-1">
                Exchange to system
              </div>
            </div>
          </div>
          
          <!-- Arbitrage Opportunity Alert -->
          <div *ngIf="absValue(mexcLag()) > 2000 && absValue(priceDiff()) > 10" 
               class="mt-4 p-4 bg-emerald-900 border border-emerald-500 rounded">
            <div class="text-emerald-400 font-bold">ARBITRAGE OPPORTUNITY!</div>
            <div class="text-sm text-emerald-300 mt-2">
              Lag: {{ mexcLag() }}ms | Price diff: {{ priceDiff() | number:'1.2-2' }} ({{ priceDiffPercent() | number:'1.3-3' }}%)
            </div>
          </div>
        </div>
        
        <!-- Settings and Stats Grid -->
        <div class="mt-8 grid grid-cols-1 lg:grid-cols-2 gap-8">
          <!-- Trading Settings -->
          <app-trading-settings 
            (settingsChanged)="onSettingsChanged($event)"
            (applyClicked)="onApplySettings($event)">
          </app-trading-settings>
          
          <!-- Trading Stats -->
          <app-trading-stats></app-trading-stats>
        </div>
        
        <!-- Positions Panel -->
        <div class="mt-8">
          <app-positions-panel [positions]="positionsData"></app-positions-panel>
        </div>
        
        <!-- History Table - moved to bottom with no auto-scroll -->
        <div class="mt-8 mb-8">
          <app-history-table [history]="historyData"></app-history-table>
        </div>
        
        <!-- Footer -->
        <div class="mt-8 text-center text-gray-500 text-sm">
          <p>Real-time data via WebSocket | Latency: {{ latency() }}ms</p>
        </div>
      </div>
    </div>
  `,
  styles: []
})
export class AppComponent implements OnDestroy {
  constructor(private marketDataService: MarketDataService) {}
  
  // Computed signals from market data
  binancePrice = computed(() => this.marketDataService.data().binance);
  mexcPrice = computed(() => this.marketDataService.data().mexc);
  spread = computed(() => this.marketDataService.data().spread);
  latency = computed(() => this.marketDataService.data().latency_ms);
  isStale = computed(() => this.marketDataService.data().is_stale);
  mexcLag = computed(() => this.marketDataService.data().mexc_lag_ms);
  realLag = computed(() => this.marketDataService.data().real_lag_ms);
  priceDiff = computed(() => this.marketDataService.data().price_diff);
  priceDiffPercent = computed(() => this.marketDataService.data().price_diff_percent);
  historyData = computed(() => this.marketDataService.historyData());
  positionsData = computed(() => this.marketDataService.positionsData());
  
  currentSettings: TradingSettings = {
    exchangeBalance: 100,
    positionSizePercent: 10,
    leverage: 200,
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
  
  onSettingsChanged(settings: TradingSettings) {
    this.currentSettings = settings;
    this.marketDataService.updateSettings(settings);
  }
  
  onApplySettings(settings: {strategy: any, capital: any}) {
    console.log('Applying settings:', settings);
    this.marketDataService.applyStrategySettings(settings.strategy);
    this.marketDataService.applyCapitalSettings(settings.capital);
  }
  
  // Helper method for absolute value
  absValue(val: number): number {
    return Math.abs(val);
  }
  
  ngOnDestroy(): void {
    this.marketDataService.disconnect();
  }
}
