import { Component, Input, Signal } from '@angular/core';
import { CommonModule } from '@angular/common';

export interface Position {
  id: string;
  entry_price: number;
  current_price: number;
  quantity: number;
  side: 'Long' | 'Short';
  entry_time: number;
  initial_impulse: number;
  trailing_stop: number;
  highest_price: number;
  lowest_price: number;
  status: 'Open' | 'Closed' | 'StopLoss' | 'TakeProfit';
}

@Component({
  selector: 'app-positions-panel',
  standalone: true,
  imports: [CommonModule],
  template: `
    <div class="card">
      <div class="flex justify-between items-center mb-4">
        <h3 class="text-gray-400 text-sm uppercase tracking-wide">
          Open Positions - MEXC Only (Simulation)
        </h3>
        <div class="flex gap-2">
          <span class="text-xs bg-blue-900 text-blue-400 px-2 py-1 rounded">
            Binance = Indicator
          </span>
          <span class="text-xs bg-emerald-900 text-emerald-400 px-2 py-1 rounded">
            MEXC = Trading (0% fee)
          </span>
          <span class="text-xs bg-yellow-900 text-yellow-400 px-2 py-1 rounded">
            DEMO MODE
          </span>
        </div>
      </div>
      
      <div *ngIf="positions().length === 0" class="text-center py-8 text-gray-500">
        <div class="text-4xl mb-2">📊</div>
        <div class="font-bold">No open positions</div>
        <div class="text-sm mt-2">Strategy: Follow Binance impulse, trade on MEXC</div>
        <div class="text-xs mt-1">Waiting for:</div>
        <div class="text-xs">• Binance impulse ≥ 0.05%</div>
        <div class="text-xs">• MEXC lag ≥ 1000ms</div>
        <div class="text-xs mt-2 text-emerald-400">Zero fees on MEXC!</div>
      </div>
      
      <div *ngIf="positions().length > 0" class="space-y-4">
        <div *ngFor="let pos of positions()" 
             class="bg-slate-900 rounded p-4 border-l-4"
             [class.border-emerald-500]="pos.side === 'Long'"
             [class.border-red-500]="pos.side === 'Short'">
          
          <!-- Header -->
          <div class="flex justify-between items-start mb-3">
            <div>
              <span class="font-bold text-lg" 
                    [class.text-emerald-400]="pos.side === 'Long'"
                    [class.text-red-400]="pos.side === 'Short'">
                {{ pos.side }}
              </span>
              <span class="text-gray-500 text-sm ml-2">
                {{ formatTime(pos.entry_time) }}
              </span>
            </div>
            <div class="text-right">
              <div class="text-xs text-gray-500">PnL</div>
              <div class="font-bold text-lg" 
                   [class.text-emerald-400]="calculatePnL(pos) > 0"
                   [class.text-red-400]="calculatePnL(pos) < 0">
                {{ calculatePnL(pos) | number:'1.3-3' }}%
              </div>
            </div>
          </div>
          
          <!-- Details Grid -->
          <div class="grid grid-cols-3 gap-3 text-sm font-mono">
            <div>
              <div class="text-gray-500 text-xs">Entry</div>
              <div class="text-white">{{ pos.entry_price | number:'1.2-2' }}</div>
            </div>
            <div>
              <div class="text-gray-500 text-xs">Current</div>
              <div class="text-white">{{ pos.current_price | number:'1.2-2' }}</div>
            </div>
            <div>
              <div class="text-gray-500 text-xs">Stop</div>
              <div class="text-yellow-400">{{ pos.trailing_stop | number:'1.2-2' }}</div>
            </div>
          </div>
          
          <!-- Progress Bar -->
          <div class="mt-3">
            <div class="flex justify-between text-xs text-gray-500 mb-1">
              <span>Stop</span>
              <span>Entry</span>
              <span>{{ pos.side === 'Long' ? 'High' : 'Low' }}</span>
            </div>
            <div class="h-2 bg-slate-700 rounded-full overflow-hidden">
              <div class="h-full bg-gradient-to-r"
                   [class.from-red-500]="pos.side === 'Long'"
                   [class.via-yellow-500]="pos.side === 'Long'"
                   [class.to-emerald-500]="pos.side === 'Long'"
                   [class.from-emerald-500]="pos.side === 'Short'"
                   [class.via-yellow-500]="pos.side === 'Short'"
                   [class.to-red-500]="pos.side === 'Short'"
                   [style.width.%]="getProgressPercent(pos)">
              </div>
            </div>
          </div>
          
          <!-- Stats -->
          <div class="mt-3 pt-3 border-t border-gray-700 grid grid-cols-3 gap-2 text-xs">
            <div>
              <span class="text-gray-500">Impulse:</span>
              <span class="text-white ml-1">{{ pos.initial_impulse | number:'1.3-3' }}%</span>
            </div>
            <div>
              <span class="text-gray-500">Qty:</span>
              <span class="text-white ml-1">{{ pos.quantity }}</span>
            </div>
            <div>
              <span class="text-gray-500">Status:</span>
              <span class="text-emerald-400 ml-1">{{ pos.status }}</span>
            </div>
          </div>
        </div>
      </div>
      
      <!-- Summary -->
      <div *ngIf="positions().length > 0" 
           class="mt-4 pt-4 border-t border-gray-700 grid grid-cols-2 gap-4 text-sm">
        <div>
          <div class="text-gray-500">Total Positions</div>
          <div class="text-white font-bold text-xl">{{ positions().length }}</div>
        </div>
        <div>
          <div class="text-gray-500">Total PnL</div>
          <div class="font-bold text-xl"
               [class.text-emerald-400]="getTotalPnL() > 0"
               [class.text-red-400]="getTotalPnL() < 0">
            {{ getTotalPnL() | number:'1.3-3' }}%
          </div>
        </div>
      </div>
    </div>
  `,
  styles: []
})
export class PositionsPanelComponent {
  @Input({ required: true }) positions!: Signal<Position[]>;
  
  formatTime(timestamp: number): string {
    const date = new Date(timestamp);
    return date.toLocaleTimeString('en-US', { 
      hour12: false,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    });
  }
  
  calculatePnL(pos: Position): number {
    if (pos.side === 'Long') {
      return ((pos.current_price - pos.entry_price) / pos.entry_price) * 100;
    } else {
      return ((pos.entry_price - pos.current_price) / pos.entry_price) * 100;
    }
  }
  
  getProgressPercent(pos: Position): number {
    if (pos.side === 'Long') {
      const range = pos.highest_price - pos.trailing_stop;
      if (range === 0) return 50;
      const progress = pos.current_price - pos.trailing_stop;
      return Math.max(0, Math.min(100, (progress / range) * 100));
    } else {
      const range = pos.trailing_stop - pos.lowest_price;
      if (range === 0) return 50;
      const progress = pos.trailing_stop - pos.current_price;
      return Math.max(0, Math.min(100, (progress / range) * 100));
    }
  }
  
  getTotalPnL(): number {
    return this.positions().reduce((sum, pos) => sum + this.calculatePnL(pos), 0);
  }
}
