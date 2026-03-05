import { Component, Input, Signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MarketData } from '../services/market-data.service';

@Component({
  selector: 'app-history-table',
  standalone: true,
  imports: [CommonModule],
  template: `
    <div class="card">
      <h3 class="text-gray-400 text-sm uppercase tracking-wide mb-4">
        Trading History (Last 50 States)
      </h3>
      
      <div class="overflow-x-auto max-h-96 overflow-y-auto" style="scroll-behavior: auto;">
        <table class="w-full text-sm font-mono">
          <thead class="text-gray-500 border-b border-gray-700 sticky top-0 bg-slate-800 z-10">
            <tr>
              <th class="text-left p-2">Time</th>
              <th class="text-right p-2">Binance</th>
              <th class="text-right p-2">MEXC</th>
              <th class="text-right p-2">Diff</th>
              <th class="text-right p-2">Lag (ms)</th>
              <th class="text-right p-2">Profit %</th>
              <th class="text-center p-2">Opportunity</th>
            </tr>
          </thead>
          <tbody>
            <tr *ngFor="let item of history(); let i = index" 
                [class]="item.is_scalping_opportunity ? 'bg-emerald-900 bg-opacity-20' : i % 2 === 0 ? 'bg-slate-800' : 'bg-slate-850'">
              <td class="p-2 text-gray-400">
                {{ formatTime(item.system_timestamp) }}
              </td>
              <td class="p-2 text-right text-white">
                {{ item.binance | number:'1.2-2' }}
              </td>
              <td class="p-2 text-right text-white">
                {{ item.mexc | number:'1.2-2' }}
              </td>
              <td class="p-2 text-right" [class]="item.price_diff > 0 ? 'text-emerald-400' : 'text-red-400'">
                {{ item.price_diff | number:'1.2-2' }}
              </td>
              <td class="p-2 text-right" [class]="absValue(item.mexc_lag_ms) > 2000 ? 'text-emerald-400 font-bold' : absValue(item.mexc_lag_ms) > 1000 ? 'text-yellow-400' : 'text-gray-400'">
                {{ item.mexc_lag_ms }}
              </td>
              <td class="p-2 text-right" [class]="item.potential_profit_percent > 0.1 ? 'text-emerald-400 font-bold' : 'text-gray-400'">
                {{ item.potential_profit_percent | number:'1.3-3' }}%
              </td>
              <td class="p-2 text-center">
                <span *ngIf="item.is_scalping_opportunity" class="text-emerald-400 text-lg">✓</span>
                <span *ngIf="!item.is_scalping_opportunity" class="text-gray-600">-</span>
              </td>
            </tr>
            <tr *ngIf="history().length === 0">
              <td colspan="7" class="p-4 text-center text-gray-500">
                Waiting for data...
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      
      <!-- Summary -->
      <div class="mt-4 pt-4 border-t border-gray-700 grid grid-cols-4 gap-4 text-sm">
        <div>
          <div class="text-gray-500">Total Opportunities</div>
          <div class="text-emerald-400 font-bold text-xl">
            {{ countOpportunities() }}
          </div>
        </div>
        <div>
          <div class="text-gray-500">Avg Profit Potential</div>
          <div class="text-white font-bold text-xl">
            {{ avgProfit() | number:'1.3-3' }}%
          </div>
        </div>
        <div>
          <div class="text-gray-500">Max Profit Seen</div>
          <div class="text-emerald-400 font-bold text-xl">
            {{ maxProfit() | number:'1.3-3' }}%
          </div>
        </div>
        <div>
          <div class="text-gray-500">Max Lag Record</div>
          <div [class]="maxLag() > 3000 ? 'text-red-400 font-bold text-xl animate-pulse' : 'text-yellow-400 font-bold text-xl'">
            {{ maxLag() }}ms
          </div>
          <div class="text-xs text-gray-600 mt-1">
            {{ maxLagTime() }}
          </div>
        </div>
      </div>
    </div>
  `,
  styles: []
})
export class HistoryTableComponent {
  @Input({ required: true }) history!: Signal<MarketData[]>;
  
  private maxLagRecord = 0;
  private maxLagRecordTime = '';
  
  formatTime(timestamp: number): string {
    const date = new Date(timestamp);
    return date.toLocaleTimeString('en-US', { 
      hour12: false,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    });
  }
  
  absValue(val: number): number {
    return Math.abs(val);
  }
  
  countOpportunities(): number {
    return this.history().filter(item => item.is_scalping_opportunity).length;
  }
  
  avgProfit(): number {
    const opportunities = this.history().filter(item => item.is_scalping_opportunity);
    if (opportunities.length === 0) return 0;
    const sum = opportunities.reduce((acc, item) => acc + item.potential_profit_percent, 0);
    return sum / opportunities.length;
  }
  
  maxProfit(): number {
    const opportunities = this.history().filter(item => item.is_scalping_opportunity);
    if (opportunities.length === 0) return 0;
    return Math.max(...opportunities.map(item => item.potential_profit_percent));
  }
  
  maxLag(): number {
    const currentHistory = this.history();
    if (currentHistory.length === 0) return this.maxLagRecord;
    
    const currentMaxLag = Math.max(...currentHistory.map(item => Math.abs(item.mexc_lag_ms)));
    
    // Update record if new max found
    if (currentMaxLag > this.maxLagRecord) {
      this.maxLagRecord = currentMaxLag;
      const maxItem = currentHistory.find(item => Math.abs(item.mexc_lag_ms) === currentMaxLag);
      if (maxItem) {
        this.maxLagRecordTime = this.formatTime(maxItem.system_timestamp);
      }
    }
    
    return this.maxLagRecord;
  }
  
  maxLagTime(): string {
    return this.maxLagRecordTime ? `at ${this.maxLagRecordTime}` : '';
  }
}
