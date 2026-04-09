import { Component, Input, OnChanges, SimpleChanges, Signal, computed } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MarketDataService, MarketData, TradingStats, TradeRecord } from '../services/market-data.service';
import { Position } from './positions-panel.component';
import { PriceCardComponent } from './price-card.component';
import { SpreadVisualizerComponent } from './spread-visualizer.component';
import { HistoryTableComponent } from './history-table.component';
import { PositionsPanelComponent } from './positions-panel.component';
import { TradingStatsComponent } from './trading-stats.component';

/**
 * Самостоятельный дашборд одного торгового инструмента (BTC/ETH/SOL).
 * Просто переиспользует все существующие блоки (price cards, spread visualizer,
 * lag analysis, positions, history, stats), но с данными конкретного символа.
 */
@Component({
  selector: 'app-symbol-dashboard',
  standalone: true,
  imports: [
    CommonModule,
    PriceCardComponent,
    SpreadVisualizerComponent,
    HistoryTableComponent,
    PositionsPanelComponent,
    TradingStatsComponent
  ],
  template: `
    <section class="border-t border-slate-700 pt-8">
      <!-- Section header -->
      <div class="flex items-baseline justify-between mb-6">
        <h2 class="text-3xl font-bold text-white">
          {{ symbol }}/USDT
        </h2>
        <div class="text-sm text-gray-500">
          Binance Futures vs MEXC Futures
        </div>
      </div>

      <!-- Main Grid: Binance / MEXC / Spread -->
      <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
        <app-price-card
          [exchange]="'Binance ' + symbol"
          [price]="binancePrice">
        </app-price-card>

        <app-price-card
          [exchange]="'MEXC ' + symbol"
          [price]="mexcPrice">
        </app-price-card>

        <app-spread-visualizer
          [spread]="spread"
          [latency]="latency"
          [isStale]="isStale">
        </app-spread-visualizer>
      </div>

      <!-- Market Statistics -->
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

      <!-- Lag Analysis (анализ стаканов/лагов) -->
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

      <!-- Trading Stats -->
      <div class="mt-8">
        <app-trading-stats
          [stats]="tradingStats"
          [recentTrades]="recentTrades">
        </app-trading-stats>
      </div>

      <!-- Positions Panel -->
      <div class="mt-8">
        <app-positions-panel [positions]="positions"></app-positions-panel>
      </div>

      <!-- History Table -->
      <div class="mt-8 mb-8">
        <app-history-table [history]="history"></app-history-table>
      </div>
    </section>
  `,
  styles: []
})
export class SymbolDashboardComponent implements OnChanges {
  @Input({ required: true }) symbol!: string;

  // Сигналы данных по символу, инициализируются в ngOnChanges
  binancePrice!: Signal<number>;
  mexcPrice!: Signal<number>;
  spread!: Signal<number>;
  latency!: Signal<number>;
  isStale!: Signal<boolean>;
  mexcLag!: Signal<number>;
  realLag!: Signal<number>;
  priceDiff!: Signal<number>;
  priceDiffPercent!: Signal<number>;
  history!: Signal<MarketData[]>;
  positions!: Signal<Position[]>;
  tradingStats!: Signal<TradingStats | null>;
  recentTrades!: Signal<TradeRecord[]>;

  constructor(private marketDataService: MarketDataService) {}

  ngOnChanges(changes: SimpleChanges): void {
    if (changes['symbol']) {
      const s = this.symbol;
      const data = this.marketDataService.symbolData(s);
      this.binancePrice = computed(() => data().binance);
      this.mexcPrice = computed(() => data().mexc);
      this.spread = computed(() => data().spread);
      this.latency = computed(() => data().latency_ms);
      this.isStale = computed(() => data().is_stale);
      this.mexcLag = computed(() => data().mexc_lag_ms);
      this.realLag = computed(() => data().real_lag_ms);
      this.priceDiff = computed(() => data().price_diff);
      this.priceDiffPercent = computed(() => data().price_diff_percent);
      this.history = this.marketDataService.symbolHistory(s);
      this.positions = this.marketDataService.symbolPositions(s);
      this.tradingStats = this.marketDataService.symbolTradingStats(s);
      this.recentTrades = this.marketDataService.symbolRecentTrades(s);
    }
  }

  absValue(val: number): number {
    return Math.abs(val);
  }
}
