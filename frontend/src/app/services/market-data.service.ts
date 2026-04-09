import { Injectable, Signal, computed, signal } from '@angular/core';
import { Position } from '../components/positions-panel.component';

export const TRADING_SYMBOLS = ['BTC', 'ETH', 'SOL'] as const;
export type TradingSymbol = typeof TRADING_SYMBOLS[number];

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

const DEFAULT_MARKET_DATA: MarketData = {
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
};

@Injectable({
  providedIn: 'root'
})
export class MarketDataService {
  private ws: WebSocket | null = null;

  // Состояние, разбитое по символам
  private marketBySymbol = signal<Record<string, MarketData>>({});
  private historyBySymbol = signal<Record<string, MarketData[]>>({});
  private positionsBySymbol = signal<Record<string, Position[]>>({});
  private tradingStatsBySymbol = signal<Record<string, TradingStats | null>>({});
  private recentTradesBySymbol = signal<Record<string, TradeRecord[]>>({});
  private systemStateSignal = signal<SystemState | null>(null);

  public systemState = computed(() => this.systemStateSignal());

  private readonly MAX_HISTORY = 50;

  // Кэш сигналов по символам (чтобы computed не пересоздавались на каждый вызов)
  private symbolDataCache: Record<string, Signal<MarketData>> = {};
  private symbolHistoryCache: Record<string, Signal<MarketData[]>> = {};
  private symbolPositionsCache: Record<string, Signal<Position[]>> = {};
  private symbolStatsCache: Record<string, Signal<TradingStats | null>> = {};
  private symbolTradesCache: Record<string, Signal<TradeRecord[]>> = {};

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

  constructor() {
    this.connect();
  }

  /** Сигнал с рыночными данными конкретного символа (BTC/ETH/SOL) */
  public symbolData(label: string): Signal<MarketData> {
    if (!this.symbolDataCache[label]) {
      this.symbolDataCache[label] = computed(() =>
        this.marketBySymbol()[label] ?? DEFAULT_MARKET_DATA
      );
    }
    return this.symbolDataCache[label];
  }

  /** Сигнал истории котировок по символу */
  public symbolHistory(label: string): Signal<MarketData[]> {
    if (!this.symbolHistoryCache[label]) {
      this.symbolHistoryCache[label] = computed(() =>
        this.historyBySymbol()[label] ?? []
      );
    }
    return this.symbolHistoryCache[label];
  }

  /** Сигнал позиций по символу */
  public symbolPositions(label: string): Signal<Position[]> {
    if (!this.symbolPositionsCache[label]) {
      this.symbolPositionsCache[label] = computed(() =>
        this.positionsBySymbol()[label] ?? []
      );
    }
    return this.symbolPositionsCache[label];
  }

  /** Сигнал торговой статистики по символу */
  public symbolTradingStats(label: string): Signal<TradingStats | null> {
    if (!this.symbolStatsCache[label]) {
      this.symbolStatsCache[label] = computed(() =>
        this.tradingStatsBySymbol()[label] ?? null
      );
    }
    return this.symbolStatsCache[label];
  }

  /** Сигнал истории сделок по символу */
  public symbolRecentTrades(label: string): Signal<TradeRecord[]> {
    if (!this.symbolTradesCache[label]) {
      this.symbolTradesCache[label] = computed(() =>
        this.recentTradesBySymbol()[label] ?? []
      );
    }
    return this.symbolTradesCache[label];
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

        // Системное состояние
        if (data.type === 'systemState') {
          this.systemStateSignal.set({
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

        // Composite multi-symbol сообщение
        if (data.type === 'marketData' && data.symbols) {
          this.handleCompositeMessage(data.symbols);
          return;
        }
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

  /** Обрабатывает composite сообщение со всеми символами */
  private handleCompositeMessage(symbols: Record<string, MarketData>): void {
    const marketNext = { ...this.marketBySymbol() };
    const historyNext = { ...this.historyBySymbol() };
    const positionsNext = { ...this.positionsBySymbol() };
    const statsNext = { ...this.tradingStatsBySymbol() };
    const tradesNext = { ...this.recentTradesBySymbol() };

    for (const label of Object.keys(symbols)) {
      const md = symbols[label];
      marketNext[label] = md;

      // История
      const existingHistory = historyNext[label] ?? [];
      const newHistory = [md, ...existingHistory];
      if (newHistory.length > this.MAX_HISTORY) {
        newHistory.length = this.MAX_HISTORY;
      }
      historyNext[label] = newHistory;

      // Позиции
      if (md.positions) {
        positionsNext[label] = md.positions;
      }

      // Trading stats
      if (md.trading_stats) {
        statsNext[label] = md.trading_stats;
      }

      // Recent trades
      if (md.recent_trades) {
        tradesNext[label] = md.recent_trades;
      }
    }

    this.marketBySymbol.set(marketNext);
    this.historyBySymbol.set(historyNext);
    this.positionsBySymbol.set(positionsNext);
    this.tradingStatsBySymbol.set(statsNext);
    this.recentTradesBySymbol.set(tradesNext);
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
