import { Component, Signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MarketDataService, TradingStats, TradeRecord } from '../services/market-data.service';

@Component({
  selector: 'app-trading-stats',
  standalone: true,
  imports: [CommonModule],
  template: `
    <div class="card">
      <h3 class="text-gray-400 text-sm uppercase tracking-wide mb-4">
        Trading Statistics
      </h3>
      
      <div *ngIf="stats(); else noStats">
        <!-- Balance Info -->
        <div class="mb-4 p-4 bg-gradient-to-r from-emerald-900 to-blue-900 rounded">
          <div class="grid grid-cols-2 gap-4">
            <div>
              <div class="text-gray-300 text-xs">Current Balance</div>
              <div class="text-white font-bold text-2xl">
                \${{ stats()!.current_balance | number:'1.2-2' }}
              </div>
            </div>
            <div>
              <div class="text-gray-300 text-xs">Total Return</div>
              <div [class]="stats()!.total_return_percent > 0 ? 'text-emerald-400' : 'text-red-400'" 
                   class="font-bold text-2xl">
                {{ stats()!.total_return_percent > 0 ? '+' : '' }}{{ stats()!.total_return_percent | number:'1.2-2' }}%
              </div>
            </div>
          </div>
        </div>
        
        <!-- Main Stats Grid -->
        <div class="grid grid-cols-2 md:grid-cols-4 gap-4 mb-4">
          <div class="bg-slate-900 p-3 rounded">
            <div class="text-gray-500 text-xs">Total Trades</div>
            <div class="text-white font-bold text-2xl">{{ stats()!.total_trades }}</div>
          </div>
          <div class="bg-slate-900 p-3 rounded">
            <div class="text-gray-500 text-xs">Win Rate</div>
            <div [class]="stats()!.win_rate >= 60 ? 'text-emerald-400' : stats()!.win_rate >= 50 ? 'text-yellow-400' : 'text-red-400'" 
                 class="font-bold text-2xl">
              {{ stats()!.win_rate | number:'1.0-0' }}%
            </div>
          </div>
          <div class="bg-slate-900 p-3 rounded">
            <div class="text-gray-500 text-xs">Total PnL</div>
            <div [class]="stats()!.total_pnl > 0 ? 'text-emerald-400' : 'text-red-400'" 
                 class="font-bold text-2xl">
              {{ stats()!.total_pnl > 0 ? '+' : '' }}\${{ stats()!.total_pnl | number:'1.2-2' }}
            </div>
          </div>
          <div class="bg-slate-900 p-3 rounded">
            <div class="text-gray-500 text-xs">Avg Trade</div>
            <div [class]="stats()!.avg_profit > 0 ? 'text-emerald-400' : 'text-red-400'" 
                 class="font-bold text-2xl">
              {{ stats()!.avg_profit > 0 ? '+' : '' }}{{ stats()!.avg_profit | number:'1.3-3' }}%
            </div>
          </div>
        </div>
        
        <!-- Performance Metrics -->
        <div class="grid grid-cols-3 gap-4 mb-4">
          <div class="bg-slate-900 p-3 rounded">
            <div class="text-gray-500 text-xs">Best Trade</div>
            <div class="text-emerald-400 font-bold">
              +{{ stats()!.best_trade | number:'1.3-3' }}%
            </div>
          </div>
          <div class="bg-slate-900 p-3 rounded">
            <div class="text-gray-500 text-xs">Worst Trade</div>
            <div class="text-red-400 font-bold">
              {{ stats()!.worst_trade | number:'1.3-3' }}%
            </div>
          </div>
          <div class="bg-slate-900 p-3 rounded">
            <div class="text-gray-500 text-xs">Win/Loss</div>
            <div class="text-white font-bold">
              {{ stats()!.winning_trades }}/{{ stats()!.losing_trades }}
            </div>
          </div>
        </div>
        
        <!-- Profit Projection -->
        <div class="p-4 bg-gradient-to-r from-emerald-900 to-blue-900 rounded mb-4">
          <div class="text-emerald-400 text-sm font-bold mb-2">
            Projected Earnings (if continued)
          </div>
          <div class="grid grid-cols-3 gap-4 text-center">
            <div>
              <div class="text-gray-300 text-xs">Daily</div>
              <div class="text-white font-bold text-lg">
                {{ projectedDaily() | number:'1.2-2' }}
              </div>
            </div>
            <div>
              <div class="text-gray-300 text-xs">Monthly</div>
              <div class="text-white font-bold text-lg">
                {{ projectedMonthly() | number:'1.2-2' }}
              </div>
            </div>
            <div>
              <div class="text-gray-300 text-xs">Yearly</div>
              <div class="text-white font-bold text-lg">
                {{ projectedYearly() | number:'1.0-0' }}
              </div>
            </div>
          </div>
          <div class="text-xs text-gray-400 mt-2 text-center">
            Based on {{ stats()!.total_trades }} trades with {{ stats()!.avg_profit | number:'1.3-3' }}% average profit
          </div>
        </div>
        
        <!-- Recent Trades -->
        <div>
          <div class="text-gray-400 text-xs uppercase mb-2">Recent Closed Trades</div>
          <div class="space-y-2 max-h-64 overflow-y-auto">
            <div *ngFor="let trade of recentTrades()" 
                 class="bg-slate-900 p-2 rounded flex justify-between items-center text-sm">
              <div class="flex items-center gap-2">
                <span [class]="trade.side === 'Long' ? 'text-emerald-400' : 'text-red-400'" 
                      class="font-bold">
                  {{ trade.side }}
                </span>
                <span class="text-gray-500 text-xs">{{ formatTime(trade.exit_time) }}</span>
              </div>
              <div class="flex items-center gap-3">
                <span class="text-gray-400 text-xs">
                  \${{ trade.entry_price | number:'1.2-2' }} → \${{ trade.exit_price | number:'1.2-2' }}
                </span>
                <span [class]="trade.pnl_percent > 0 ? 'text-emerald-400' : 'text-red-400'" 
                      class="font-bold">
                  {{ trade.pnl_percent > 0 ? '+' : '' }}{{ trade.pnl_percent | number:'1.3-3' }}%
                </span>
              </div>
            </div>
            <div *ngIf="recentTrades().length === 0" 
                 class="text-center text-gray-500 py-4 text-sm">
              No closed trades yet
            </div>
          </div>
        </div>
      </div>
      
      <ng-template #noStats>
        <div class="text-center text-gray-500 py-8">
          <div class="text-4xl mb-2">📊</div>
          <div>No trading data yet</div>
          <div class="text-xs mt-2">Statistics will appear after first trade</div>
        </div>
      </ng-template>
    </div>
  `,
  styles: []
})
export class TradingStatsComponent {
  stats: Signal<TradingStats | null>;
  recentTrades: Signal<TradeRecord[]>;
  
  constructor(private marketDataService: MarketDataService) {
    this.stats = this.marketDataService.tradingStatsData;
    this.recentTrades = this.marketDataService.recentTradesData;
  }
  
  projectedDaily(): number {
    const s = this.stats();
    if (!s || s.avg_profit === 0) return 0;
    const tradesPerDay = 15; // Conservative estimate
    return s.current_balance * (s.avg_profit / 100) * tradesPerDay;
  }
  
  projectedMonthly(): number {
    return this.projectedDaily() * 30;
  }
  
  projectedYearly(): number {
    return this.projectedDaily() * 365;
  }
  
  formatTime(timestamp: number): string {
    const date = new Date(timestamp);
    return date.toLocaleTimeString('en-US', { 
      hour12: false,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    });
  }
}
