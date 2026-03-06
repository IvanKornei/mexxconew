import { Injectable } from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { Subject } from 'rxjs';
import { Position } from '../components/positions-panel.component';

export interface MarketData {
  binance: number;
  mexc: number;
  spread: number;
  is_stale: boolean;
  latency_ms: number;
  binance_timestamp: number;
  mexc_timestamp: number;
  system_timestamp: number;
  mexc_lag_ms: number;
  price_diff: number;
  price_diff_percent: number;
  binance_received_at: number;
  mexc_received_at: number;
  real_lag_ms: number;
  is_scalping_opportunity: boolean;
  potential_profit_percent: number;
  // Позиции
  positions?: Position[];
  total_pnl?: number;
  open_positions_count?: number;
  // Статистика из базы данных
  trading_stats?: TradingStats;
  recent_trades?: TradeRecord[];
}

export interface TradingStats {
  total_trades: number;
  winning_trades: number;
  losing_trades: number;
  total_pnl: number;
  win_rate: number;
  avg_profit: number;
  best_trade: number;
  worst_trade: number;
  current_balance: number;
  initial_balance: number;
  total_return_percent: number;
}

export interface TradeRecord {
  id: number;
  position_id: string;
  side: string;
  entry_price: number;
  exit_price: number;
  quantity: number;
  entry_time: number;
  exit_time: number;
  pnl: number;
  pnl_percent: number;
  status: string;
  initial_impulse: number;
}

export interface SystemState {
  is_running: boolean;
  mode: string;
  last_updated: number;
  trading_settings: {
    capital: number;
    position_size_percent: number;
    leverage: number;
    max_positions: number;
    momentum_threshold: number;
    quick_exit_timeout: number;
    take_profit_percent: number;
    stop_loss_percent: number;
    momentum_weight: number;
    lag_weight: number;
    price_diff_weight: number;
  };
}

@Injectable({
  providedIn: 'root'
})
export class MarketDataService {
  private ws: WebSocket | null = null;
  private dataSubject = new Subject<MarketData>();
  private historySubject = new Subject<MarketData[]>();
  private positionsSubject = new Subject<Position[]>();
  private tradingStatsSubject = new Subject<TradingStats | null>();
  private recentTradesSubject = new Subject<TradeRecord[]>();
  private systemStateSubject = new Subject<SystemState | null>();
  
  public systemState$ = this.systemStateSubject.asObservable();
  
  private history: MarketData[] = [];
  private positions: Position[] = [];
  private readonly MAX_HISTORY = 50;
  
  // Trading settings (can be updated)
  public tradingSettings = {
    capital: 10,
    leverage: 200,
    maxPositions: 2,
    momentumThreshold: 0.03,
    quickExitTimeout: 2000,
    takeProfitPercent: 0.04,
    stopLossPercent: 0.02
  };
  
  get positionSize(): number {
    const effectiveCapital = this.tradingSettings.capital * this.tradingSettings.leverage;
    // Use 50% of effective capital per position
    return (effectiveCapital * 0.5) / 67000; // Approximate BTC price
  }
  
  public data = toSignal(this.dataSubject, {
    initialValue: {
      binance: 0,
      mexc: 0,
      spread: 0,
      is_stale: true,
      latency_ms: 0,
      binance_timestamp: 0,
      mexc_timestamp: 0,
      system_timestamp: 0,
      mexc_lag_ms: 0,
      price_diff: 0,
      price_diff_percent: 0,
      binance_received_at: 0,
      mexc_received_at: 0,
      real_lag_ms: 0,
      is_scalping_opportunity: false,
      potential_profit_percent: 0
    }
  });
  
  public historyData = toSignal(this.historySubject, {
    initialValue: []
  });
  
  public positionsData = toSignal(this.positionsSubject, {
    initialValue: []
  });
  
  public tradingStatsData = toSignal(this.tradingStatsSubject, {
    initialValue: null
  });
  
  public recentTradesData = toSignal(this.recentTradesSubject, {
    initialValue: []
  });
  
  constructor() {
    this.connect();
  }
  
  private connect(): void {
    const wsUrl = 'ws://localhost:3001/ws';
    console.log('Connecting to WebSocket:', wsUrl);
    
    this.ws = new WebSocket(wsUrl);
    
    this.ws.onopen = () => {
      console.log('WebSocket connected');
    };
    
    this.ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        
        // Проверяем тип сообщения
        if (data.type === 'systemState') {
          // Обновление состояния системы
          this.systemStateSubject.next({
            is_running: data.is_running,
            mode: data.mode,
            last_updated: data.last_updated,
            trading_settings: data.trading_settings
          });
          return;
        }
        
        if (data.type === 'error') {
          console.error('Server error:', data.message);
          return;
        }
        
        // Обычное сообщение с данными рынка
        const marketData: MarketData = data;
        this.dataSubject.next(marketData);
        
        // Обновляем позиции если они пришли
        if (marketData.positions) {
          this.positions = marketData.positions;
          this.positionsSubject.next([...this.positions]);
        }
        
        // Обновляем статистику из базы данных
        if (marketData.trading_stats) {
          this.tradingStatsSubject.next(marketData.trading_stats);
        }
        
        // Обновляем историю сделок
        if (marketData.recent_trades) {
          this.recentTradesSubject.next(marketData.recent_trades);
        }
        
        this.history.unshift(marketData);
        if (this.history.length > this.MAX_HISTORY) {
          this.history.pop();
        }
        this.historySubject.next([...this.history]);
        
        this.processMarketData(marketData);
      } catch (error) {
        console.error('Failed to parse WebSocket message:', error);
      }
    };
    
    this.ws.onerror = (error) => {
      console.error('WebSocket error:', error);
    };
    
    this.ws.onclose = () => {
      console.log('WebSocket disconnected, reconnecting in 2s...');
      setTimeout(() => this.connect(), 2000);
    };
  }
  
  private processMarketData(data: MarketData): void {
    this.positions = this.positions.map(pos => {
      const updated = { ...pos };
      updated.current_price = data.mexc;
      
      if (pos.side === 'Long' && data.mexc > pos.highest_price) {
        updated.highest_price = data.mexc;
        const profit = updated.highest_price - updated.entry_price;
        updated.trailing_stop = updated.entry_price + (profit * 0.5);
      } else if (pos.side === 'Short' && data.mexc < pos.lowest_price) {
        updated.lowest_price = data.mexc;
        const profit = updated.entry_price - updated.lowest_price;
        updated.trailing_stop = updated.entry_price - (profit * 0.5);
      }
      
      return updated;
    });
    
    this.positions = this.positions.filter(pos => {
      const stopLossPercent = this.tradingSettings.stopLossPercent;
      
      if (pos.side === 'Long') {
        const lossPercent = ((pos.current_price - pos.entry_price) / pos.entry_price) * 100;
        if (pos.current_price <= pos.trailing_stop || lossPercent <= -stopLossPercent) {
          console.log(`Position ${pos.id} closed by stop loss`);
          return false;
        }
      } else {
        const lossPercent = ((pos.entry_price - pos.current_price) / pos.entry_price) * 100;
        if (pos.current_price >= pos.trailing_stop || lossPercent <= -stopLossPercent) {
          console.log(`Position ${pos.id} closed by stop loss`);
          return false;
        }
      }
      return true;
    });
    
    // Check for new entry with settings
    if (this.positions.length < this.tradingSettings.maxPositions && 
        data.is_scalping_opportunity &&
        Math.abs(data.price_diff_percent) >= this.tradingSettings.momentumThreshold &&
        Math.abs(data.mexc_lag_ms) >= 200) {
      
      const side: 'Long' | 'Short' = data.binance > data.mexc ? 'Long' : 'Short';
      
      const newPosition: Position = {
        id: `pos_${Date.now()}`,
        entry_price: data.mexc,
        current_price: data.mexc,
        quantity: this.positionSize,
        side,
        entry_time: data.system_timestamp,
        initial_impulse: Math.abs(data.price_diff_percent),
        trailing_stop: data.mexc,
        highest_price: data.mexc,
        lowest_price: data.mexc,
        status: 'Open'
      };
      
      this.positions.push(newPosition);
      console.log(`New ${side} position opened at ${data.mexc} (${this.positionSize} BTC)`);
    }
    
    this.positionsSubject.next([...this.positions]);
  }
  
  updateSettings(settings: any) {
    this.tradingSettings = { ...this.tradingSettings, ...settings };
    console.log('Trading settings updated:', this.tradingSettings);
  }
  
  // Отправляет настройки стратегии на бэкенд
  applyStrategySettings(settings: {
    momentum_weight: number;
    lag_weight: number;
    price_diff_weight: number;
    momentum_threshold: number;
    quick_exit_timeout_ms: number;
  }): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      const command = {
        type: 'updateSettings',
        settings
      };
      this.ws.send(JSON.stringify(command));
      console.log('Strategy settings sent to backend:', settings);
    } else {
      console.error('WebSocket not connected, cannot send settings');
    }
  }
  
  // Отправляет настройки капитала на бэкенд
  applyCapitalSettings(settings: {
    capital: number;
    position_size_percent: number;
    leverage: number;
    max_positions: number;
  }): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      const command = {
        type: 'updateCapital',
        ...settings
      };
      this.ws.send(JSON.stringify(command));
      console.log('Capital settings sent to backend:', settings);
    } else {
      console.error('WebSocket not connected, cannot send capital settings');
    }
  }
  
  disconnect(): void {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }
  
  // Методы управления системой
  startTrading(): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      const command = { type: 'startTrading' };
      this.ws.send(JSON.stringify(command));
      console.log('Start trading command sent');
    }
  }
  
  stopTrading(): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      const command = { type: 'stopTrading' };
      this.ws.send(JSON.stringify(command));
      console.log('Stop trading command sent');
    }
  }
  
  switchMode(mode: 'Emulation' | 'Live'): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      const command = { 
        type: 'switchMode',
        mode: mode.toLowerCase()
      };
      this.ws.send(JSON.stringify(command));
      console.log('Switch mode command sent:', mode);
    }
  }
  
  getSystemState(): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      const command = { type: 'getSystemState' };
      this.ws.send(JSON.stringify(command));
    }
  }
}
