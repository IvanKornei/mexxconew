import { Component, signal, inject, effect, computed, OnDestroy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MarketDataService } from '../services/market-data.service';

/** Порог «протухания» WS-соединения, после которого блокируем опасные операции. */
const WS_STALE_THRESHOLD_MS = 5000;

@Component({
  selector: 'app-control-panel',
  standalone: true,
  imports: [CommonModule],
  template: `
    <div class="control-panel bg-gray-800 rounded-lg p-4 mb-4">
      <!-- Status Display -->
      <div class="flex items-center justify-between mb-4">
        <div class="flex items-center gap-3">
          <div class="status-indicator" [class.running]="isRunning()" [class.stopped]="!isRunning()">
            <div class="pulse"></div>
          </div>
          <div>
            <div class="text-sm text-gray-400">Статус системы</div>
            <div class="text-lg font-semibold" [class.text-green-400]="isRunning()" [class.text-gray-400]="!isRunning()">
              {{ isRunning() ? 'Торговля активна' : 'Торговля остановлена' }}
            </div>
          </div>
        </div>
        
        <!-- Mode Badge -->
        <div class="mode-badge" [class.emulation]="currentMode() === 'Emulation'" [class.live]="currentMode() === 'Live'">
          <span class="mode-icon">{{ currentMode() === 'Emulation' ? '🧪' : '🔴' }}</span>
          <span class="mode-text">{{ currentMode() === 'Emulation' ? 'Эмуляция' : 'Live Торговля' }}</span>
        </div>
      </div>
      
      <!-- Connection Warning -->
      @if (!isConnectionHealthy()) {
        <div class="connection-warning mb-4">
          <span class="warning-icon">📡</span>
          <span>
            {{ wsConnected() ? 'Данные устарели' : 'Нет соединения с сервером' }} —
            управление торговлей заблокировано до восстановления связи.
          </span>
        </div>
      }

      <!-- Control Buttons -->
      <div class="flex gap-3">
        <button
          (click)="toggleTrading()"
          class="control-btn"
          [class.btn-stop]="isRunning()"
          [class.btn-start]="!isRunning()"
          [disabled]="isProcessing() || !canOperate()">
          <span class="btn-icon">{{ isRunning() ? '⏸' : '▶' }}</span>
          <span>{{ isRunning() ? 'Остановить' : 'Запустить' }}</span>
        </button>

        <button
          (click)="switchMode()"
          class="control-btn btn-mode"
          [disabled]="isRunning() || isProcessing() || !canOperate()">
          <span class="btn-icon">🔄</span>
          <span>Переключить режим</span>
        </button>
      </div>
      
      <!-- Warning for Live Mode -->
      @if (showLiveWarning()) {
        <div class="warning-box mt-4">
          <div class="warning-icon">⚠️</div>
          <div class="warning-content">
            <div class="warning-title">Переключение в Live режим</div>
            <div class="warning-text">
              Вы собираетесь переключиться в режим реальной торговли. 
              Все сделки будут отправляться на биржу. Продолжить?
            </div>
            <div class="warning-actions">
              <button (click)="confirmModeSwitch()" class="btn-confirm">Подтвердить</button>
              <button (click)="cancelModeSwitch()" class="btn-cancel">Отмена</button>
            </div>
          </div>
        </div>
      }
    </div>
  `,
  styles: [`
    .control-panel {
      border: 1px solid rgba(255, 255, 255, 0.1);
    }
    
    .status-indicator {
      position: relative;
      width: 16px;
      height: 16px;
      border-radius: 50%;
      
      &.running {
        background: #10b981;
        box-shadow: 0 0 10px rgba(16, 185, 129, 0.5);
      }
      
      &.stopped {
        background: #6b7280;
      }
      
      .pulse {
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        border-radius: 50%;
        animation: pulse 2s ease-in-out infinite;
      }
    }
    
    @keyframes pulse {
      0%, 100% {
        box-shadow: 0 0 0 0 rgba(16, 185, 129, 0.7);
      }
      50% {
        box-shadow: 0 0 0 8px rgba(16, 185, 129, 0);
      }
    }
    
    .mode-badge {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 8px 16px;
      border-radius: 8px;
      font-weight: 600;
      
      &.emulation {
        background: rgba(34, 197, 94, 0.1);
        color: #22c55e;
        border: 1px solid rgba(34, 197, 94, 0.3);
      }
      
      &.live {
        background: rgba(239, 68, 68, 0.1);
        color: #ef4444;
        border: 1px solid rgba(239, 68, 68, 0.3);
      }
      
      .mode-icon {
        font-size: 18px;
      }
    }
    
    .control-btn {
      flex: 1;
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 8px;
      padding: 12px 24px;
      border-radius: 8px;
      font-weight: 600;
      transition: all 0.2s;
      border: none;
      cursor: pointer;
      
      &:disabled {
        opacity: 0.5;
        cursor: not-allowed;
      }
      
      .btn-icon {
        font-size: 18px;
      }
      
      &.btn-start {
        background: linear-gradient(135deg, #10b981 0%, #059669 100%);
        color: white;
        
        &:hover:not(:disabled) {
          transform: translateY(-2px);
          box-shadow: 0 4px 12px rgba(16, 185, 129, 0.4);
        }
      }
      
      &.btn-stop {
        background: linear-gradient(135deg, #ef4444 0%, #dc2626 100%);
        color: white;
        
        &:hover:not(:disabled) {
          transform: translateY(-2px);
          box-shadow: 0 4px 12px rgba(239, 68, 68, 0.4);
        }
      }
      
      &.btn-mode {
        background: rgba(59, 130, 246, 0.1);
        color: #3b82f6;
        border: 1px solid rgba(59, 130, 246, 0.3);
        
        &:hover:not(:disabled) {
          background: rgba(59, 130, 246, 0.2);
          transform: translateY(-2px);
        }
      }
    }
    
    .connection-warning {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 10px 14px;
      background: rgba(234, 179, 8, 0.12);
      border: 1px solid rgba(234, 179, 8, 0.35);
      border-radius: 8px;
      color: #facc15;
      font-size: 13px;

      .warning-icon {
        font-size: 18px;
      }
    }

    .warning-box {
      display: flex;
      gap: 12px;
      padding: 16px;
      background: rgba(239, 68, 68, 0.1);
      border: 1px solid rgba(239, 68, 68, 0.3);
      border-radius: 8px;
      
      .warning-icon {
        font-size: 24px;
        flex-shrink: 0;
      }
      
      .warning-content {
        flex: 1;
      }
      
      .warning-title {
        font-weight: 600;
        color: #ef4444;
        margin-bottom: 4px;
      }
      
      .warning-text {
        font-size: 14px;
        color: #d1d5db;
        margin-bottom: 12px;
      }
      
      .warning-actions {
        display: flex;
        gap: 8px;
      }
      
      .btn-confirm, .btn-cancel {
        padding: 8px 16px;
        border-radius: 6px;
        font-weight: 600;
        border: none;
        cursor: pointer;
        transition: all 0.2s;
      }
      
      .btn-confirm {
        background: #ef4444;
        color: white;
        
        &:hover {
          background: #dc2626;
        }
      }
      
      .btn-cancel {
        background: rgba(107, 114, 128, 0.2);
        color: #d1d5db;
        
        &:hover {
          background: rgba(107, 114, 128, 0.3);
        }
      }
    }
  `]
})
export class ControlPanelComponent implements OnDestroy {
  private marketDataService = inject(MarketDataService);

  isRunning = signal(false);
  currentMode = signal<'Emulation' | 'Live'>('Emulation');
  isProcessing = signal(false);
  showLiveWarning = signal(false);

  // Тикер времени для пересчёта staleness (signals пересчитаются каждую секунду)
  private nowTick = signal(Date.now());
  private tickInterval = setInterval(() => this.nowTick.set(Date.now()), 1000);

  // Пробрасываем для шаблона
  wsConnected = this.marketDataService.wsConnected;

  /** Свежее ли WS-соединение: сокет открыт и пакет пришёл < 5 секунд назад. */
  isConnectionHealthy = computed(() => {
    if (!this.marketDataService.wsConnected()) return false;
    const last = this.marketDataService.lastMessageAt();
    if (last === 0) return false;
    return this.nowTick() - last < WS_STALE_THRESHOLD_MS;
  });

  /** Разрешено ли отправлять команды управления торговлей. */
  canOperate = computed(() => this.isConnectionHealthy());

  constructor() {
    // Реактивно отслеживаем состояние системы через signal effect
    effect(() => {
      const state = this.marketDataService.systemState();
      if (state) {
        this.isRunning.set(state.is_running);
        this.currentMode.set(state.mode as 'Emulation' | 'Live');
      }
    });
  }

  ngOnDestroy(): void {
    clearInterval(this.tickInterval);
  }
  
  toggleTrading() {
    if (this.isProcessing()) return;
    // Страховка: даже если кнопка осталась активной по какой-то причине,
    // запрещаем отправку команд при отсутствии свежих данных.
    if (!this.canOperate()) {
      console.warn('Cannot toggle trading — WS connection is stale or down');
      return;
    }

    this.isProcessing.set(true);

    if (this.isRunning()) {
      this.marketDataService.stopTrading();
    } else {
      this.marketDataService.startTrading();
    }

    // Сбрасываем флаг обработки через 1 секунду
    setTimeout(() => this.isProcessing.set(false), 1000);
  }

  switchMode() {
    if (this.isRunning() || this.isProcessing()) return;
    if (!this.canOperate()) {
      console.warn('Cannot switch mode — WS connection is stale or down');
      return;
    }
    
    // Если переключаемся в Live - показываем предупреждение
    if (this.currentMode() === 'Emulation') {
      this.showLiveWarning.set(true);
    } else {
      // Переключение из Live в Emulation - без предупреждения
      this.performModeSwitch();
    }
  }
  
  confirmModeSwitch() {
    this.showLiveWarning.set(false);
    this.performModeSwitch();
  }
  
  cancelModeSwitch() {
    this.showLiveWarning.set(false);
  }
  
  private performModeSwitch() {
    this.isProcessing.set(true);
    
    const newMode = this.currentMode() === 'Emulation' ? 'Live' : 'Emulation';
    this.marketDataService.switchMode(newMode);
    
    setTimeout(() => this.isProcessing.set(false), 1000);
  }
}
