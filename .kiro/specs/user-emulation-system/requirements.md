# Requirements Document

## Introduction

Гибридная система эмуляции пользовательского поведения для работы с биржей MEXC Futures. Система сочетает высокоскоростное API-взаимодействие для торговли (<1мс латентность) с периодической эмуляцией браузерного поведения для поддержания легитимности сессии. Использует продвинутые техники маскировки на уровнях TLS, HTTP/2 и поведенческой симуляции для предотвращения блокировок при скальпинге и арбитраже.

## Glossary

- **EmulationSystem**: Гибридная система эмуляции для MEXC на базе Rust
- **SessionInitializer**: Компонент для первоначальной инициализации сессии через браузерную эмуляцию
- **TLSEmulator**: Компонент для эмуляции TLS-отпечатков браузеров (JA3/JA4)
- **BehaviorSimulator**: Компонент для периодической имитации человеческого поведения (UI-действия)
- **TradingExecutor**: Высокоскоростной компонент для выполнения сделок через WebSocket API
- **SessionPersistence**: Механизм сохранения cookies, токенов и состояния TLS между запросами
- **MEXCEmulator**: Специализированный адаптер для работы с MEXC Futures
- **PeriodicActivitySimulator**: Компонент для фоновой имитации активности пользователя
- **WebSocketEmulator**: Компонент для WebSocket-соединений с сохранением браузерных отпечатков
- **ParameterManager**: Компонент для управления торговыми параметрами (размер позиции, стопы)
- **TradingMode**: Режим работы системы (Hybrid или Aggressive)
- **HumanJitter**: Компонент для добавления случайных задержек, имитирующих человеческую реакцию

## Requirements

### Requirement 1

**User Story:** Как трейдер-арбитражник, я хочу эмулировать полный сценарий подготовки к торговле перед запуском скрипта, чтобы MEXC воспринимал меня как реального пользователя

#### Acceptance Criteria

1. WHEN THE SessionInitializer запускается, THE SessionInitializer SHALL эмулировать открытие браузера Chrome 136 с корректными TLS-отпечатками
2. WHEN THE SessionInitializer загружает страницу MEXC, THE SessionInitializer SHALL выполнить загрузку всех статических ресурсов (CSS, JS, изображения) для имитации реального браузера
3. WHEN THE SessionInitializer проходит аутентификацию, THE BehaviorSimulator SHALL имитировать ввод логина и пароля с задержками 100-300мс между символами
4. WHEN THE SessionInitializer проходит аутентификацию, THE BehaviorSimulator SHALL имитировать движение мыши по кривой Безье к кнопке входа с последующим кликом
5. AFTER успешной аутентификации, THE BehaviorSimulator SHALL эмулировать навигацию к разделу Futures Trading с задержкой 2-4 секунды
6. WHEN THE BehaviorSimulator открывает торговую страницу, THE BehaviorSimulator SHALL имитировать просмотр графика BTC/USDT с прокруткой и движением мыши в течение 3-7 секунд
7. THE BehaviorSimulator SHALL имитировать проверку баланса и открытых позиций перед началом торговли
8. THE BehaviorSimulator SHALL имитировать настройку торговых параметров (размер позиции, leverage) через UI с кликами и вводом значений
9. THE SessionInitializer SHALL завершать полную pre-trading эмуляцию за время 15-30 секунд
10. WHEN THE SessionInitializer получает cookies и токены, THE SessionPersistence SHALL сохранить их для использования в TradingExecutor


### Requirement 2

**User Story:** Как трейдер-арбитражник, я хочу мониторить Binance для сигналов и выполнять сделки на MEXC с минимальной латентностью, используя уже установленную легитимную сессию

#### Acceptance Criteria

1. THE TradingExecutor SHALL подключаться к Binance WebSocket для мониторинга цен BTC/USDT без эмуляции (чистое API)
2. WHEN THE TradingExecutor обнаруживает отставание MEXC от Binance по алгоритму, THE TradingExecutor SHALL отправить ордер на MEXC через WebSocket за время менее 1 миллисекунды
3. THE TradingExecutor SHALL использовать cookies и токены из SessionPersistence для аутентификации WebSocket-соединения с MEXC
4. THE TradingExecutor SHALL поддерживать постоянное WebSocket-соединение с MEXC без переподключений во время активной торговли
5. WHEN THE TradingExecutor обнаруживает разрыв WebSocket-соединения с MEXC, THE TradingExecutor SHALL автоматически переподключиться с экспоненциальной задержкой (1s, 2s, 4s)
6. THE TradingExecutor SHALL отправлять торговые команды на MEXC без эмуляции UI-действий для минимизации латентности

### Requirement 3

**User Story:** Как трейдер-скальпер, я хочу периодически имитировать человеческую активность в фоне, чтобы поддерживать легитимность сессии без влияния на скорость торговли

#### Acceptance Criteria

1. THE PeriodicActivitySimulator SHALL выполнять фоновые UI-действия каждые 5-15 минут (случайный интервал)
2. WHEN THE PeriodicActivitySimulator выполняет фоновое действие, THE BehaviorSimulator SHALL имитировать проверку баланса, просмотр открытых позиций или истории сделок
3. THE PeriodicActivitySimulator SHALL выполнять фоновые действия в отдельном потоке без блокировки TradingExecutor
4. WHEN THE PeriodicActivitySimulator имитирует UI-действие, THE BehaviorSimulator SHALL использовать стохастические задержки 50-200мс между кликами
5. THE PeriodicActivitySimulator SHALL логировать все фоновые действия для анализа паттернов активности

### Requirement 4

**User Story:** Как трейдер-скальпер, я хочу использовать стандартные торговые параметры (размер позиции, стопы), настроенные один раз через UI-эмуляцию, чтобы избежать подозрительных паттернов

#### Acceptance Criteria

1. WHEN THE ParameterManager инициализируется, THE BehaviorSimulator SHALL имитировать ввод размера позиции через UI MEXC с задержками между символами
2. THE ParameterManager SHALL сохранять стандартные параметры (размер позиции, стоп-лосс %, тейк-профит %) для переиспользования
3. WHEN THE TradingExecutor открывает позицию, THE TradingExecutor SHALL использовать сохраненные параметры без повторного ввода через UI
4. THE ParameterManager SHALL обновлять параметры через UI-эмуляцию не чаще одного раза в час
5. WHEN THE ParameterManager обновляет параметры, THE BehaviorSimulator SHALL имитировать клики по полям настроек и ввод новых значений


### Requirement 5

**User Story:** Как трейдер-скальпер, я хочу эмулировать TLS и HTTP/2 отпечатки браузера Chrome, чтобы WebSocket-соединения не детектировались как боты

#### Acceptance Criteria

1. THE TLSEmulator SHALL генерировать JA3-отпечаток, идентичный Chrome 136 для Windows
2. THE TLSEmulator SHALL использовать BoringSSL для низкоуровневого контроля над TLS-рукопожатием
3. WHEN THE WebSocketEmulator выполняет HTTP Upgrade, THE WebSocketEmulator SHALL использовать те же TLS-отпечатки, что и SessionInitializer
4. THE TLSEmulator SHALL генерировать HTTP/2 кадры SETTINGS с параметрами Chrome 136
5. THE TLSEmulator SHALL сохранять оригинальный регистр HTTP-заголовков согласно спецификации Chrome

### Requirement 6

**User Story:** Как трейдер-скальпер, я хочу сохранять состояние сессии между перезапусками системы, чтобы избежать повторной инициализации через браузерную эмуляцию

#### Acceptance Criteria

1. THE SessionPersistence SHALL сохранять cookies, токены и TLS session tickets в зашифрованном файле
2. WHEN THE EmulationSystem запускается, THE SessionPersistence SHALL проверять валидность сохраненной сессии
3. IF сохраненная сессия валидна, THEN THE EmulationSystem SHALL пропустить SessionInitializer и использовать TradingExecutor напрямую
4. IF сохраненная сессия невалидна или истекла, THEN THE EmulationSystem SHALL запустить SessionInitializer для создания новой сессии
5. THE SessionPersistence SHALL обновлять сохраненное состояние каждые 5 минут во время активной торговли

### Requirement 7

**User Story:** Как трейдер-скальпер, я хочу работать с домашнего IP без прокси для максимального Trust Score, с возможностью добавления residential прокси при необходимости

#### Acceptance Criteria

1. THE EmulationSystem SHALL по умолчанию использовать домашний IP пользователя без прокси
2. THE EmulationSystem SHALL ограничивать частоту фоновых UI-действий до 10 запросов в час с домашнего IP
3. THE EmulationSystem SHALL поддерживать опциональную конфигурацию residential прокси через config.toml
4. WHERE residential прокси настроены, THE EmulationSystem SHALL использовать Sticky Session для сохранения IP на протяжении всей торговой сессии
5. WHEN THE EmulationSystem получает ошибку 429 (Too Many Requests), THE EmulationSystem SHALL экспоненциально увеличивать задержки между запросами


### Requirement 8

**User Story:** Как трейдер-скальпер, я хочу использовать специализированные Rust-библиотеки для эмуляции, чтобы достичь баланса между скоростью и скрытностью

#### Acceptance Criteria

1. THE SessionInitializer SHALL использовать библиотеку eoka для браузерной автоматизации при инициализации сессии
2. THE TradingExecutor SHALL использовать библиотеку wreq с BoringSSL для высокоскоростных WebSocket-соединений
3. THE BehaviorSimulator SHALL использовать библиотеку spider_fingerprint для генерации JavaScript-фингерпринтов
4. THE TradingExecutor SHALL обеспечивать латентность обработки данных менее 1мс после получения через WebSocket
5. THE EmulationSystem SHALL поддерживать конфигурацию через файл config.toml с параметрами профилей браузеров и торговых лимитов

### Requirement 9

**User Story:** Как трейдер-скальпер, я хочу интегрировать эмуляцию с существующим MEXCEmulator без изменения бизнес-логики арбитража

#### Acceptance Criteria

1. THE MEXCEmulator SHALL реализовывать тот же интерфейс ExchangeClient, что и существующий коннектор MEXC
2. WHEN THE MEXCEmulator инициализируется, THE SessionInitializer SHALL выполнить браузерную эмуляцию для получения легитимной сессии
3. WHEN THE MEXCEmulator получает запрос на подписку на рыночные данные, THE TradingExecutor SHALL использовать WebSocket-соединение с сохраненными токенами
4. THE MEXCEmulator SHALL запускать PeriodicActivitySimulator в фоновом потоке без блокировки торговых операций
5. WHEN THE MEXCEmulator обнаруживает блокировку или ошибку аутентификации, THE MEXCEmulator SHALL автоматически запустить SessionInitializer для переинициализации сессии

### Requirement 10

**User Story:** Как трейдер-скальпер, я хочу мониторить эффективность эмуляции и получать алерты при блокировках, чтобы оперативно реагировать на проблемы

#### Acceptance Criteria

1. THE EmulationSystem SHALL логировать все события инициализации сессии с временными метками и результатами
2. THE EmulationSystem SHALL логировать все фоновые UI-действия PeriodicActivitySimulator для анализа паттернов
3. WHEN THE EmulationSystem обнаруживает ошибку 403, 429 или блокировку Cloudflare, THE EmulationSystem SHALL отправить алерт через систему логирования
4. THE EmulationSystem SHALL собирать метрики латентности торговых операций (p50, p95, p99)
5. THE EmulationSystem SHALL предоставлять dashboard через Angular frontend с отображением статуса сессии, количества фоновых действий и метрик латентности


### Requirement 11

**User Story:** Как трейдер-арбитражник, я хочу выбирать между гибридным и агрессивным режимами торговли, чтобы балансировать между скоростью и безопасностью

#### Acceptance Criteria

1. THE EmulationSystem SHALL поддерживать два режима работы: Hybrid (по умолчанию) и Aggressive
2. THE EmulationSystem SHALL загружать выбранный режим из config.toml параметра trading_mode
3. WHEN trading_mode установлен в Hybrid, THE TradingExecutor SHALL добавлять HumanJitter задержку 20-80мс перед отправкой каждого ордера
4. WHEN trading_mode установлен в Aggressive, THE TradingExecutor SHALL отправлять ордера без дополнительных задержек с минимальной латентностью <1мс
5. WHEN trading_mode установлен в Hybrid, THE PeriodicActivitySimulator SHALL выполнять фоновые UI-действия каждые 10-20 минут
6. WHEN trading_mode установлен в Aggressive, THE PeriodicActivitySimulator SHALL выполнять фоновые UI-действия каждые 30-60 минут
7. THE EmulationSystem SHALL логировать выбранный режим при запуске системы

### Requirement 12

**User Story:** Как трейдер-арбитражник в гибридном режиме, я хочу имитировать человеческие паттерны поведения во время торговли, чтобы минимизировать риск блокировки

#### Acceptance Criteria

1. WHEN THE TradingExecutor работает в Hybrid режиме, THE HumanJitter SHALL генерировать случайную задержку 20-80мс с нормальным распределением перед каждым ордером
2. WHEN THE TradingExecutor выполнил 15-25 сделок (случайный порог), THE PeriodicActivitySimulator SHALL имитировать проверку открытых позиций через UI
3. WHEN THE TradingExecutor работает непрерывно 2-3 часа, THE PeriodicActivitySimulator SHALL имитировать "перерыв" длительностью 2-5 минут без торговой активности
4. THE PeriodicActivitySimulator SHALL случайным образом имитировать просмотр других торговых пар (ETH/USDT, SOL/USDT) каждые 30-60 минут
5. WHEN THE TradingExecutor в Hybrid режиме отправляет ордер, THE HumanJitter SHALL варьировать минимальный интервал между сделками в диапазоне 500мс ±200мс

### Requirement 13

**User Story:** Как трейдер-арбитражник в агрессивном режиме, я хочу максимизировать скорость торговли с минимальной эмуляцией, принимая повышенный риск детектирования

#### Acceptance Criteria

1. WHEN THE TradingExecutor работает в Aggressive режиме, THE TradingExecutor SHALL отправлять ордера немедленно без HumanJitter задержек
2. WHEN THE TradingExecutor работает в Aggressive режиме, THE TradingExecutor SHALL поддерживать минимальный интервал между сделками 100мс для предотвращения rate limiting
3. WHEN THE TradingExecutor работает в Aggressive режиме, THE PeriodicActivitySimulator SHALL выполнять минимальные фоновые действия (только проверка баланса) каждые 30-60 минут
4. THE EmulationSystem в Aggressive режиме SHALL пропускать имитацию "перерывов" для непрерывной торговли
5. WHEN THE EmulationSystem в Aggressive режиме получает ошибку 429 или блокировку, THE EmulationSystem SHALL автоматически переключиться на Hybrid режим и выполнить полную переинициализацию сессии
