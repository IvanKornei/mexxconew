import { Component, OnDestroy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MarketDataService, TRADING_SYMBOLS } from './services/market-data.service';
import { PriceCardComponent } from './components/price-card.component';
import { SpreadVisualizerComponent } from './components/spread-visualizer.component';
import { HistoryTableComponent } from './components/history-table.component';
import { PositionsPanelComponent } from './components/positions-panel.component';
import { TradingSettingsComponent, TradingSettings } from './components/trading-settings.component';
import { TradingStatsComponent } from './components/trading-stats.component';
import { ControlPanelComponent } from './components/control-panel.component';
import { SymbolDashboardComponent } from './components/symbol-dashboard.component';

@Component({
  selector: 'app-root',
  standalone: true,
  imports: [
    CommonModule,
    SymbolDashboardComponent,
    TradingSettingsComponent,
    ControlPanelComponent
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
            Multi-Pair Spread Monitoring - Binance Futures vs MEXC Futures
          </p>
        </div>

        <!-- Control Panel -->
        <app-control-panel></app-control-panel>

        <!-- Global Settings (общие для всех символов) -->
        <div class="mt-8">
          <app-trading-settings
            (settingsChanged)="onSettingsChanged($event)"
            (applyClicked)="onApplySettings($event)">
          </app-trading-settings>
        </div>

        <!-- Symbol Dashboards (BTC → ETH → SOL) -->
        <div *ngFor="let symbol of symbols" class="mt-12">
          <app-symbol-dashboard [symbol]="symbol"></app-symbol-dashboard>
        </div>

        <!-- Footer -->
        <div class="mt-8 text-center text-gray-500 text-sm">
          <p>Real-time data via WebSocket | Symbols: {{ symbols.join(' / ') }}</p>
        </div>
      </div>
    </div>
  `,
  styles: []
})
export class AppComponent implements OnDestroy {
  readonly symbols = [...TRADING_SYMBOLS];

  constructor(private marketDataService: MarketDataService) {}

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

  ngOnDestroy(): void {
    this.marketDataService.disconnect();
  }
}
