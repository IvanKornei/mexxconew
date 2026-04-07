# Implementation Plan

## Фаза 1: Фундамент и базовые утилиты

- [x] 1. Настройка базовой инфраструктуры проекта


  - Добавить необходимые Rust crates в Cargo.toml (eoka, wreq, boring, spider_fingerprint, aes-gcm, rand_distr, tokio, serde, tracing)
  - Создать структуру директорий: src/emulation/ с подмодулями tls/, behavior/, session/, trading/, persistence/
  - Настроить конфигурацию emulation секции в config.toml с параметрами режимов и rate limits
  - Создать .kiro/emulation/ директорию для хранения зашифрованных сессий
  - Настроить структурированное логирование через tracing с уровнями ERROR, WARN, INFO, DEBUG, TRACE
  - _Requirements: 8.5, 10.1_

- [x] 2. Реализация системы обработки ошибок


  - Создать src/emulation/errors.rs с enum EmulationError (SessionInitFailed, BrowserError, WebSocketError, AuthError, RateLimitError, SessionExpired, BlockedError)
  - Реализовать Display и Error traits для EmulationError
  - Создать type alias Result<T> = std::result::Result<T, EmulationError>
  - _Requirements: 2.5, 7.5, 13.5_

- [x] 3. Реализация конфигурации и TradingMode



  - Создать src/emulation/config.rs с структурами EmulationConfig, RateLimitConfig, ProxyConfig
  - Создать enum TradingMode (Hybrid, Aggressive) с методами загрузки из config.toml
  - Реализовать загрузку конфигурации из config.toml при старте
  - Реализовать логирование выбранного TradingMode при запуске системы
  - _Requirements: 11.1, 11.2, 11.7_

## Фаза 2: Низкоуровневые компоненты эмуляции

- [x] 4. Реализация TLSEmulator для браузерных отпечатков


  - Создать src/emulation/tls/mod.rs с структурой TLSEmulator и enum BrowserProfile
  - Реализовать generate_ja3_fingerprint() для Chrome 145 с корректными cipher suites и extensions
  - Реализовать generate_ja4_fingerprint() с алфавитной сортировкой параметров
  - Реализовать configure_http2_settings() для генерации SETTINGS frames Chrome 145
  - Настроить BoringSSL для низкоуровневого контроля TLS handshake
  - _Requirements: 5.1, 5.2, 5.4, 5.5_

- [x] 5. Реализация HumanJitter для случайных задержек


  - Создать src/emulation/trading/human_jitter.rs с структурой HumanJitter
  - Реализовать new() с инициализацией нормального распределения (mean=50мс, std_dev=15мс)
  - Реализовать apply_jitter() для генерации задержки в диапазоне 20-80мс с использованием rand_distr
  - Обеспечить отсутствие фиксированных паттернов через стохастическое распределение
  - _Requirements: 11.3, 12.1, 12.5_

- [x] 6. Реализация BehaviorSimulator для имитации человеческого поведения



  - Создать src/emulation/behavior/mod.rs с структурой BehaviorSimulator
  - Реализовать simulate_mouse_movement() с генерацией траекторий по кривой Безье (B(t) = (1-t)³P₀ + 3(1-t)²tP₁ + 3(1-t)t²P₂ + t³P₃)
  - Реализовать simulate_typing() с случайными задержками 100-300мс между символами
  - Реализовать simulate_click() с движением мыши к элементу и задержкой перед кликом
  - Реализовать simulate_scroll() с ускорением и замедлением
  - _Requirements: 1.3, 1.4, 4.1_

## Фаза 3: Управление сессиями

- [x] 7. Реализация SessionPersistence для сохранения состояния


  - Создать src/emulation/persistence/mod.rs с структурой SessionPersistence и SessionData
  - Реализовать save_session() с сериализацией в JSON и шифрованием AES-256-GCM
  - Реализовать load_session() с расшифровкой и проверкой валидности (expires_at)
  - Реализовать is_session_valid() для проверки срока действия сессии
  - Настроить загрузку ключа шифрования из переменной окружения EMULATION_SESSION_KEY
  - Реализовать логирование событий сохранения/загрузки сессии
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 10.1_

- [x] 8. Реализация SessionInitializer для браузерной эмуляции


  - Создать src/emulation/session/mod.rs с структурой SessionInitializer
  - Интегрировать eoka browser с профилем Chrome 145 (64-bit) Windows, viewport 1920x1080
  - Интегрировать TLSEmulator для установки браузерных отпечатков
  - Реализовать initialize_session() для полного цикла: запуск браузера, навигация к MEXC login
  - Интегрировать BehaviorSimulator для эмуляции логина с задержками ввода 100-300мс
  - Реализовать эмуляцию навигации к Futures Trading с задержкой 2-4 секунды
  - Реализовать эмуляцию просмотра графика BTC/USDT с прокруткой и движением мыши 3-7 секунд
  - Реализовать эмуляцию проверки баланса и открытых позиций
  - Реализовать извлечение cookies, токенов и TLS session tickets
  - Интегрировать SessionPersistence для сохранения полученной сессии
  - Обеспечить время выполнения 15-30 секунд для полной инициализации
  - Реализовать retry логику с экспоненциальной задержкой (1s, 2s, 4s, 8s), максимум 3 попытки
  - Реализовать логирование всех событий инициализации с временными метками
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.9, 1.10, 10.1, 10.2_

- [ ] 9. Реализация ParameterManager для торговых параметров



  - Создать src/emulation/trading/parameter_manager.rs с структурой ParameterManager
  - Реализовать initialize_parameters() для эмуляции ввода размера позиции, leverage через UI
  - Интегрировать BehaviorSimulator для имитации кликов и ввода значений с задержками
  - Реализовать get_order_parameters() для получения сохраненных параметров
  - Реализовать update_parameters_if_needed() для обновления параметров раз в час через UI
  - _Requirements: 1.8, 4.1, 4.2, 4.3, 4.4, 4.5_

## Фаза 4: Торговое ядро

- [ ] 10. Реализация TradingExecutor для высокоскоростной торговли



  - Создать src/emulation/trading/executor.rs с структурой TradingExecutor
  - Реализовать connect_to_binance() для мониторинга цен через чистое WebSocket API (без эмуляции)
  - Реализовать connect_to_mexc() с использованием wreq WebSocket и сохраненных токенов из SessionData
  - Интегрировать TLSEmulator для сохранения браузерных отпечатков в WebSocket соединении
  - Реализовать execute_order() с условной интеграцией HumanJitter (только в Hybrid режиме)
  - Интегрировать ParameterManager для получения торговых параметров
  - Реализовать monitor_arbitrage() для обнаружения отставания MEXC от Binance
  - Обеспечить латентность <1мс для отправки ордера в Aggressive режиме
  - Реализовать rate limiting с минимальным интервалом 100мс между сделками в Aggressive режиме
  - Реализовать автоматическое переподключение WebSocket с экспоненциальной задержкой (1s, 2s, 4s)
  - Реализовать сбор метрик латентности торговых операций (p50, p95, p99)
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 5.3, 8.2, 8.4, 10.4, 11.4, 13.1, 13.2_

## Фаза 5: Фоновая активность и режимы

- [x] 11. Реализация PeriodicActivitySimulator для фоновой активности


  - Создать src/emulation/behavior/periodic_activity.rs с структурой PeriodicActivitySimulator
  - Реализовать run_background_loop() для выполнения фоновых действий в отдельном tokio::spawn потоке
  - Реализовать perform_random_activity() с выбором случайного действия (проверка баланса, просмотр позиций, история, другие пары)
  - Реализовать calculate_next_interval() с учетом TradingMode (10-20 мин Hybrid, 30-60 мин Aggressive)
  - Интегрировать BehaviorSimulator для имитации UI-действий с задержками 50-200мс
  - Обеспечить работу в отдельном потоке без блокировки TradingExecutor
  - Реализовать логирование всех фоновых UI-действий для анализа паттернов
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 10.2, 11.5, 11.6, 13.3_

- [x] 12. Реализация логики режимов Hybrid и Aggressive



  - Интегрировать условную логику в TradingExecutor для добавления HumanJitter только в Hybrid режиме
  - Реализовать логику "перерывов" в PeriodicActivitySimulator (2-5 минут каждые 2-3 часа) только для Hybrid режима
  - Реализовать имитацию просмотра других торговых пар (ETH/USDT, SOL/USDT) каждые 30-60 минут в Hybrid режиме
  - Реализовать имитацию проверки открытых позиций после 15-25 сделок в Hybrid режиме
  - Реализовать варьирование минимального интервала между сделками 500мс ±200мс в Hybrid режиме
  - Реализовать автоматическое переключение Aggressive → Hybrid при получении ошибки 429 или блокировки
  - Реализовать полную переинициализацию сессии через SessionInitializer при переключении режимов
  - _Requirements: 11.3, 11.4, 11.5, 11.6, 12.1, 12.2, 12.3, 12.4, 12.5, 13.4, 13.5_

## Фаза 6: Интеграция и расширенные функции

- [x] 13. Реализация поддержки прокси и IP management


  - Добавить поля ProxyConfig в EmulationConfig для параметров provider и sticky_session
  - Реализовать опциональную интеграцию residential прокси в SessionInitializer
  - Реализовать опциональную интеграцию residential прокси в TradingExecutor для WebSocket соединений
  - Реализовать Sticky Session для сохранения IP на протяжении торговой сессии
  - Обеспечить работу по умолчанию с домашнего IP без прокси
  - Реализовать ограничение частоты фоновых UI-действий до 10 запросов в час с домашнего IP
  - _Requirements: 7.1, 7.2, 7.3, 7.4_

- [x] 14. Реализация recovery стратегий для ошибок


  - Реализовать автоматический запуск SessionInitializer при AuthError или SessionExpired в TradingExecutor
  - Реализовать экспоненциальное увеличение задержек при RateLimitError
  - Реализовать переключение Aggressive → Hybrid и полную переинициализацию при BlockedError
  - Реализовать алерты через систему логирования при обнаружении ошибок 403, 429, блокировки Cloudflare
  - _Requirements: 2.5, 7.5, 10.3, 13.5_

- [ ] 15. Реализация MEXCEmulator как адаптера ExchangeClient



  - Создать src/exchanges/mexc_emulator.rs с структурой MEXCEmulator
  - Реализовать trait ExchangeClient для MEXCEmulator (совместимость с существующим интерфейсом)
  - Интегрировать SessionPersistence для проверки валидности сохраненной сессии при инициализации
  - Интегрировать SessionInitializer для инициализации сессии при создании MEXCEmulator (если сессия невалидна)
  - Интегрировать TradingExecutor для подписки на рыночные данные и выполнения ордеров
  - Запустить PeriodicActivitySimulator в фоновом tokio::spawn потоке при инициализации
  - Реализовать автоматическую переинициализацию сессии при обнаружении блокировки или ошибки аутентификации
  - Обеспечить совместимость с существующей бизнес-логикой арбитража без изменений
  - _Requirements: 6.2, 6.3, 6.4, 9.1, 9.2, 9.3, 9.4, 9.5_

## Фаза 7: Мониторинг и UI

- [x] 16. Интеграция с Angular frontend для dashboard




  - Создать WebSocket endpoint в src/api/websocket.rs для отправки статуса эмуляции
  - Реализовать отправку метрик: статус сессии, количество фоновых действий, латентность торговли (p50, p95, p99)
  - Создать Angular компонент EmulationStatusComponent в frontend/src/app/components/emulation-status/
  - Создать Angular service EmulationDataService для подключения к WebSocket endpoint
  - Реализовать отображение текущего TradingMode (Hybrid/Aggressive) с индикатором
  - Реализовать отображение времени последней инициализации сессии и срока действия
  - Реализовать отображение графика латентности (p50, p95, p99) с использованием Chart.js
  - Реализовать отображение счетчика фоновых действий за последний час
  - Применить Tailwind CSS стили в dark theme для trading terminal стиля
  - _Requirements: 10.5_

## Фаза 8: Тестирование и оптимизация (опционально)

- [ ]* 17. Написание unit тестов для критических компонентов
  - Написать тесты для TLSEmulator: проверка корректности JA3/JA4 fingerprints, валидация HTTP/2 SETTINGS
  - Написать тесты для BehaviorSimulator: проверка генерации кривых Безье, валидация диапазонов задержек
  - Написать тесты для HumanJitter: проверка нормального распределения, валидация диапазона 20-80мс
  - Написать тесты для SessionPersistence: проверка шифрования/расшифровки, валидация срока действия
  - _Requirements: 5.1, 5.4, 12.1_

- [ ]* 18. Написание integration тестов для end-to-end сценариев
  - Написать тест для полного цикла инициализации сессии с проверкой сохранения cookies и токенов
  - Написать тест для подключения к MEXC WebSocket с сохраненной сессией
  - Написать тест для переключения Aggressive → Hybrid при блокировке
  - Написать тест для автоматической переинициализации сессии при SessionExpired
  - _Requirements: 6.2, 6.3, 9.5, 13.5_

- [ ]* 19. Performance benchmarking и оптимизация
  - Создать benchmark с использованием criterion для измерения латентности отправки ордера (target <1мс Aggressive, <100мс Hybrid)
  - Создать benchmark для измерения времени инициализации сессии (target 15-30 секунд)
  - Создать benchmark для измерения overhead фоновой активности (target <1% CPU)
  - Провести load test с симуляцией 100 ордеров в минуту
  - Оптимизировать memory management с использованием Arc<RwLock> для shared state
  - Профилировать критические пути с помощью flamegraph
  - _Requirements: 1.9, 2.2, 8.4_
