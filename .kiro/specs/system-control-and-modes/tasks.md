# Implementation Plan

- [x] 1. Создать базовую инфраструктуру управления системой (Backend)



  - Создать модуль `src/core/system_manager.rs` с структурой `SystemManager`
  - Реализовать enum `TradingMode` (Emulation, Live)
  - Реализовать структуру `SystemState` с полями is_running, mode, last_updated, trading_settings
  - Реализовать методы `new()`, `start_trading()`, `stop_trading()`, `switch_mode()`
  - Реализовать методы `load_state()` и `save_state()` для работы с файлом `.kiro/system_state.json`
  - Реализовать методы `update_settings()` и `get_settings()` для управления настройками
  - Добавить валидацию настроек перед сохранением
  - Использовать атомарную запись (write to temp + rename) для безопасности
  - _Requirements: 1.1, 1.3, 5.1, 5.2, 5.3, 5.4_

- [x] 2. Создать менеджер режимов торговли (Backend)





  - Создать модуль `src/core/trading_mode_manager.rs` с структурой `TradingModeManager`
  - Реализовать методы `set_mode()`, `get_current_mode()`
  - Создать отдельные экземпляры `TradingHistory` для emulation и live режимов
  - Реализовать методы `get_stats()` и `get_trades()` с параметром режима
  - Реализовать метод `record_trade()` для записи в соответствующую БД
  - Настроить пути к БД: `emulation.db` и `live.db`
  - _Requirements: 2.1, 2.2, 3.4_

- [x] 3. Расширить Position Manager для поддержки управления (Backend)




  - Добавить поля `is_trading_enabled: Arc<RwLock<bool>>` и `execution_mode: Arc<RwLock<TradingMode>>`
  - Реализовать методы `set_trading_enabled()` и `is_trading_enabled()`
  - Реализовать методы `set_execution_mode()` и `get_execution_mode()`
  - Модифицировать `process_market_state()` для проверки `is_trading_enabled` в начале метода
  - Добавить логику выбора режима выполнения (emulation vs live) при открытии позиций
  - В режиме Emulation: только логирование без реальных ордеров
  - В режиме Live: подготовить место для интеграции с Exchange API (заглушка)
  - _Requirements: 1.1, 1.2, 1.3, 2.2, 2.3_

- [x] 4. Интегрировать System Manager в main.rs (Backend)




  - Импортировать `SystemManager` и `TradingModeManager` в `main.rs`
  - Создать экземпляр `SystemManager` после создания `PositionManager`
  - Загрузить состояние системы при запуске через `load_state()`
  - Применить загруженные настройки к `PositionManager`
  - Если режим Live и is_running=true, запросить подтверждение (логировать предупреждение)
  - Передать `SystemManager` в `WsServer` через `State`
  - _Requirements: 5.1, 5.5_

- [x] 5. Расширить WebSocket протокол для команд управления (Backend)




  - Добавить новые варианты в enum `ClientCommand`: `StartTrading`, `StopTrading`, `SwitchMode`
  - Добавить новые варианты в enum `ServerResponse`: `SystemState`, `ConfirmationRequired`, `Error`
  - Реализовать обработчики команд в функции `handle_socket`
  - Для `StartTrading`: вызвать `system_manager.start_trading()`, отправить обновление состояния
  - Для `StopTrading`: вызвать `system_manager.stop_trading()`, отправить обновление состояния
  - Для `SwitchMode`: проверить что торговля остановлена, вызвать `system_manager.switch_mode()`, отправить обновление
  - Добавить отправку `SystemState` при подключении клиента (в initial_msg)
  - При изменении настроек через `UpdateSettings` или `UpdateCapital` вызывать `system_manager.update_settings()` и `save_state()`
  - _Requirements: 1.1, 1.2, 2.4, 4.3_




- [ ] 6. Создать Control Panel компонент (Frontend)
  - Создать файл `frontend/src/app/components/control-panel.component.ts`
  - Реализовать интерфейс `SystemControlState` с полями isRunning, currentMode, lastUpdated
  - Создать signals для `isRunning` и `currentMode`
  - Реализовать метод `toggleTrading()` для отправки команд Start/Stop
  - Реализовать метод `switchMode()` с диалогом подтверждения для Live режима
  - Добавить визуальную индикацию состояния (running/stopped)
  - Использовать цветовую схему: зеленый для Emulation, красный для Live
  - Добавить стили для кнопок и индикаторов состояния
  - _Requirements: 1.5, 2.5, 4.1, 4.4_

- [ ] 7. Создать Mode Tabs компонент (Frontend)
  - Создать файл `frontend/src/app/components/mode-tabs.component.ts`
  - Реализовать signal `activeTab` для отслеживания активной вкладки
  - Реализовать метод `setActiveTab()` для переключения вкладок
  - Создать шаблон с двумя вкладками: Emulation и Live Trading
  - Условно отображать компоненты статистики и истории в зависимости от активной вкладки
  - Добавить визуальное различие вкладок (цветовая индикация)
  - Синхронизировать активную вкладку с текущим режимом системы
  - _Requirements: 3.1, 3.2, 3.3, 3.5_

- [ ] 8. Расширить Market Data Service для управления системой (Frontend)
  - Добавить интерфейсы `SystemCommand` и `SystemStateMessage`
  - Создать `systemStateSubject` и signal `systemState`
  - Реализовать методы `startTrading()`, `stopTrading()`, `switchMode()`
  - Реализовать метод `sendCommand()` для отправки команд через WebSocket
  - Добавить обработку сообщений типа `systemState` в `onmessage`
  - Обновлять `systemStateSubject` при получении обновлений состояния
  - Добавить обработку сообщений типа `confirmationRequired` и `error`
  - Сохранять полученные настройки в localStorage для персистентности
  - При подключении WebSocket загружать настройки из localStorage и отправлять на бэкенд
  - _Requirements: 1.1, 1.2, 2.4, 4.1_

- [ ] 9. Модифицировать компоненты статистики для поддержки режимов (Frontend)
  - Добавить input параметр `mode: 'emulation' | 'live'` в `TradingStatsComponent`
  - Добавить input параметр `mode: 'emulation' | 'live'` в `HistoryTableComponent`
  - Модифицировать `MarketDataService` для запроса данных по конкретному режиму



  - Добавить визуальную индикацию режима в компонентах (badge или цветовая метка)
  - Обновить отображение данных в зависимости от выбранного режима
  - _Requirements: 3.2, 3.3, 3.4_

- [ ] 10. Интегрировать компоненты управления в App Component (Frontend)
  - Импортировать `ControlPanelComponent` и `ModeTabsComponent` в `AppComponent`
  - Добавить компоненты в шаблон `AppComponent` (в верхней части страницы)
  - Подписаться на `systemState` из `MarketDataService`
  - Передать состояние системы в дочерние компоненты
  - Обновить layout для размещения панели управления и вкладок
  - Добавить обработку диалогов подтверждения на уровне App Component
  - _Requirements: 1.5, 2.5, 3.1_

- [ ] 11. Реализовать корректную обработку отрицательных лагов (Backend)
  - Проверить текущую логику вычисления лага в `PriceState`
  - Убедиться что лаг вычисляется как `mexc_timestamp - binance_timestamp`
  - В методах принятия торговых решений использовать `lag.abs()` для определения возможностей
  - Добавить комментарии объясняющие интерпретацию положительных и отрицательных лагов
  - Обновить логирование для корректного отображения знака лага
  - _Requirements: 6.1, 6.2, 6.3, 6.4_

- [ ] 12. Обновить UI для отображения отрицательных лагов (Frontend)
  - Модифицировать `SpreadVisualizerComponent` для корректного отображения отрицательных лагов
  - Добавить интерпретацию: положительный лаг = MEXC delayed, отрицательный = Binance delayed
  - Обновить цветовую индикацию в зависимости от знака лага
  - Добавить tooltip с объяснением значения лага
  - Убедиться что торговые возможности определяются по `abs(lag)`
  - _Requirements: 6.5_

- [ ] 13. Добавить обработку ошибок и edge cases (Backend)
  - Реализовать обработку поврежденного файла состояния (создать новый с defaults)
  - Добавить retry логику для сохранения состояния (повтор через 5 секунд)
  - Реализовать проверку: отклонять переключение режима при активной торговле
  - Добавить fallback на in-memory хранилище при недоступности БД
  - Добавить логирование всех ошибок с соответствующими уровнями (WARNING, ERROR)
  - Реализовать graceful degradation при ошибках
  - _Requirements: 4.3, 4.5_

- [ ] 14. Добавить обработку ошибок в UI (Frontend)
  - Реализовать отображение уведомления "Connection lost, reconnecting..." при отключении WebSocket
  - Блокировать кнопки управления при отсутствии соединения
  - Показывать сообщения об ошибках от сервера в toast notifications
  - Реализовать откат UI в предыдущее состояние при отказе операции
  - Добавить timeout 30 секунд для диалогов подтверждения
  - Показывать loading состояние во время выполнения команд
  - _Requirements: 4.1, 4.2_

- [ ]* 15. Написать unit тесты (Backend)
  - Тесты для `SystemManager::start_trading()` и `stop_trading()`
  - Тесты для `SystemManager::switch_mode()` с различными сценариями
  - Тесты для `SystemManager::save_state()` и `load_state()` с валидными и невалидными данными
  - Тесты для `TradingModeManager::record_trade()` проверяющие запись в правильную БД
  - Тесты для `PositionManager::process_market_state()` с флагом stopped
  - Тесты для обработки отрицательных лагов
  - _Requirements: All_

- [ ]* 16. Написать unit тесты (Frontend)
  - Тесты для `ControlPanelComponent.toggleTrading()`
  - Тесты для `ControlPanelComponent.switchMode()` с диалогом подтверждения
  - Тесты для `ModeTabsComponent.setActiveTab()`
  - Тесты для `MarketDataService.sendCommand()` проверяющие формирование команд
  - Тесты для сохранения и загрузки настроек из localStorage
  - _Requirements: All_

- [ ]* 17. Провести integration тесты
  - Тест полного цикла Start → Stop через UI
  - Тест переключения режимов Emulation ↔ Live
  - Тест сохранения и восстановления состояния после перезапуска
  - Тест сохранения и восстановления настроек после обновления страницы
  - Тест обработки отрицательных лагов end-to-end
  - Тест разделения данных между режимами
  - _Requirements: All_
