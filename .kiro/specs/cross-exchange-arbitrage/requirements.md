# Requirements Document

## Introduction

Система межбиржевого арбитража предназначена для мониторинга и торговли на основе ценовых расхождений между Binance Futures и MEXC Futures для пары BTC/USDT. Система работает в режиме реального времени с ультра-низкой латентностью (<1ms для обработки данных), используя Rust backend для высокопроизводительной обработки и Angular frontend для визуализации.

## Glossary

- **Arbitrage System**: Программная система для обнаружения и исполнения межбиржевого арбитража
- **Price Feed**: Поток данных о ценах торговых пар в реальном времени
- **Order Book**: Книга заявок, содержащая информацию о bid/ask ценах и объемах
- **Spread**: Разница в ценах одной торговой пары между двумя биржами
- **Execution Engine**: Компонент системы, отвечающий за размещение и управление ордерами
- **Risk Manager**: Компонент системы, контролирующий риски и лимиты позиций
- **WebSocket Connection**: Постоянное соединение для получения данных в реальном времени
- **Trading Pair**: Торговая пара криптовалют (например, BTC/USDT)
- **Latency**: Задержка между получением данных и исполнением операции
- **Balance Monitor**: Компонент для отслеживания балансов на биржах
- **Dashboard**: Angular-based веб-интерфейс для мониторинга и управления системой
- **Binance Futures Stream**: WebSocket endpoint wss://fstream.binance.com/ws/btcusdt@aggTrade
- **MEXC Futures Stream**: WebSocket endpoint wss://wbs.mexc.com/ws/public/v1/market/deal?symbol=BTC_USDT
- **Price State**: Shared state для передачи цен между async tasks
- **Spread Calculator**: Компонент для расчета процентной разницы цен между биржами

## Requirements

### Requirement 1

**User Story:** Как трейдер, я хочу получать данные о ценах в реальном времени с обеих бирж, чтобы обнаруживать арбитражные возможности с минимальной задержкой

#### Acceptance Criteria

1. WHEN THE Arbitrage System запускается, THE Price Feed SHALL установить WebSocket соединение с Binance Futures Stream на wss://fstream.binance.com/ws/btcusdt@aggTrade
2. WHEN THE Arbitrage System запускается, THE Price Feed SHALL установить WebSocket соединение с MEXC Futures Stream на wss://wbs.mexc.com/ws/public/v1/market/deal?symbol=BTC_USDT
3. WHILE THE Price Feed активен, THE Arbitrage System SHALL обрабатывать обновления цен с латентностью менее 1 миллисекунды
4. IF WebSocket Connection прерывается, THEN THE Price Feed SHALL автоматически переподключиться используя exponential backoff стратегию
5. WHEN THE Price Feed получает обновление цены, THE Arbitrage System SHALL сохранить timestamp с точностью до микросекунды

### Requirement 2

**User Story:** Как трейдер, я хочу автоматически обнаруживать арбитражные возможности, чтобы не упускать прибыльные сделки

#### Acceptance Criteria

1. WHEN THE Arbitrage System получает обновление цены, THE Spread Calculator SHALL вычислить процентный Spread по формуле ((PriceMEXC - PriceBinance) / PriceBinance) * 100
2. THE Spread Calculator SHALL нормализовать символы BTC_USDT и BTCUSDT к единому формату перед расчетом
3. THE Arbitrage System SHALL обнаружить арбитражную возможность, когда Spread превышает 0.3 процента после учета комиссий
4. THE Arbitrage System SHALL использовать Arc<RwLock> или tokio::sync::watch для передачи Price State между async tasks без блокировок
5. WHEN арбитражная возможность обнаружена, THE Arbitrage System SHALL передать сигнал в Execution Engine

### Requirement 3

**User Story:** Как трейдер, я хочу иметь готовую инфраструктуру для исполнения арбитражных сделок, чтобы в будущем автоматизировать торговлю

#### Acceptance Criteria

1. THE Arbitrage System SHALL определить trait ExchangeClient с методами place_order, cancel_order и get_balance
2. THE Arbitrage System SHALL реализовать boilerplate для MEXC ExchangeClient с Hmac-Sha256 аутентификацией
3. THE Arbitrage System SHALL загружать API ключи из .env файла для MEXC клиента
4. THE ExchangeClient SHALL предоставить асинхронные методы для размещения и отмены ордеров
5. THE ExchangeClient SHALL предоставить метод для получения баланса аккаунта

### Requirement 4

**User Story:** Как трейдер, я хочу контролировать риски и лимиты позиций, чтобы защитить капитал от потерь

#### Acceptance Criteria

1. THE Risk Manager SHALL проверить доступный баланс перед размещением каждого ордера
2. THE Risk Manager SHALL ограничить размер одной сделки до 5 процентов от общего капитала
3. WHEN общая экспозиция по одной Trading Pair превышает 20 процентов капитала, THE Risk Manager SHALL блокировать новые сделки по этой паре
4. THE Risk Manager SHALL отслеживать общую прибыль и убытки в реальном времени
5. IF общий убыток за день превышает 2 процента капитала, THEN THE Risk Manager SHALL остановить торговлю

### Requirement 5

**User Story:** Как трейдер, я хочу мониторить балансы на биржах, чтобы обеспечить достаточную ликвидность для сделок

#### Acceptance Criteria

1. THE Balance Monitor SHALL запрашивать балансы на обеих биржах каждые 10 секунд
2. WHEN баланс на любой бирже падает ниже 1000 USDT, THE Balance Monitor SHALL отправить уведомление
3. THE Balance Monitor SHALL отслеживать балансы для всех торгуемых активов
4. THE Arbitrage System SHALL использовать кэшированные данные балансов для проверки возможности сделки
5. IF запрос баланса завершается ошибкой, THEN THE Balance Monitor SHALL повторить запрос через 1 секунду

### Requirement 6

**User Story:** Как трейдер, я хочу видеть цены и спред в реальном времени через веб-интерфейс, чтобы мониторить арбитражные возможности

#### Acceptance Criteria

1. THE Dashboard SHALL быть реализован на Angular 21 с использованием Signals для управления состоянием
2. THE Dashboard SHALL отображать PriceCardComponent для каждой биржи с текущей ценой BTC/USDT
3. THE Dashboard SHALL отображать SpreadVisualizerComponent с индикатором спреда (зеленый для положительного, красный для отрицательного)
4. THE Dashboard SHALL использовать Tailwind CSS с темной темой в стиле трейдингового терминала
5. THE Dashboard SHALL подключаться к Rust backend через WebSocket для получения обновлений в реальном времени

### Requirement 7

**User Story:** Как трейдер, я хочу безопасно хранить API ключи бирж, чтобы защитить доступ к моим аккаунтам

#### Acceptance Criteria

1. THE Arbitrage System SHALL загружать API ключи из зашифрованного конфигурационного файла при запуске
2. THE Arbitrage System SHALL хранить API ключи в памяти без записи в логи
3. THE Arbitrage System SHALL использовать отдельные API ключи для каждой биржи
4. THE Arbitrage System SHALL проверить валидность API ключей при инициализации
5. IF API ключ невалиден, THEN THE Arbitrage System SHALL остановить запуск и вывести сообщение об ошибке

### Requirement 8

**User Story:** Как трейдер, я хочу логировать все операции системы, чтобы анализировать производительность и отлаживать проблемы

#### Acceptance Criteria

1. THE Arbitrage System SHALL использовать tracing crate для структурированного логирования
2. THE Arbitrage System SHALL записывать все обнаруженные арбитражные возможности в лог с уровнем INFO
3. THE Arbitrage System SHALL записывать ошибки подключения и API в лог с уровнем ERROR
4. THE Arbitrage System SHALL ротировать лог-файлы при достижении размера 100 мегабайт
5. THE Arbitrage System SHALL сохранять лог-файлы за последние 30 дней

### Requirement 9

**User Story:** Как трейдер, я хочу чтобы система предоставляла WebSocket API для фронтенда, чтобы получать данные в реальном времени

#### Acceptance Criteria

1. THE Arbitrage System SHALL запустить WebSocket сервер на базе axum framework
2. THE WebSocket Server SHALL транслировать текущие цены с обеих бирж клиентам
3. THE WebSocket Server SHALL транслировать вычисленный спред клиентам
4. THE WebSocket Server SHALL использовать JSON формат для сообщений
5. THE WebSocket Server SHALL поддерживать множественные одновременные подключения клиентов

### Requirement 10

**User Story:** Как трейдер, я хочу чтобы система следовала принципам чистого кода, чтобы легко поддерживать и расширять функциональность

#### Acceptance Criteria

1. THE Arbitrage System SHALL следовать принципам SOLID в архитектуре
2. THE Arbitrage System SHALL использовать модульную структуру с четким разделением ответственности
3. THE Arbitrage System SHALL использовать serde с оптимизированными атрибутами для быстрого парсинга JSON
4. THE Arbitrage System SHALL минимизировать использование Zone.js в Angular для повышения производительности
5. THE Arbitrage System SHALL использовать async/await паттерн с tokio runtime для всех I/O операций

### Requirement 11

**User Story:** Как HFT трейдер, я хочу отслеживать латентность системы в реальном времени, чтобы обнаруживать проблемы с производительностью

#### Acceptance Criteria

1. THE Arbitrage System SHALL вычислять разницу между системным временем и timestamp биржи для каждого обновления цены
2. THE Arbitrage System SHALL отображать latency в миллисекундах в Dashboard
3. WHEN latency превышает 100 миллисекунд, THE Arbitrage System SHALL записать WARNING в лог
4. THE Arbitrage System SHALL отслеживать среднюю, минимальную и максимальную латентность за последние 60 секунд
5. THE Dashboard SHALL отображать Spread Health виджет с индикатором латентности

### Requirement 12

**User Story:** Как HFT трейдер, я хочу обнаруживать устаревшие данные от бирж, чтобы не принимать решения на основе неактуальной информации

#### Acceptance Criteria

1. WHEN данные от биржи не обновлялись более 300 миллисекунд, THE Arbitrage System SHALL пометить состояние как "Stale"
2. THE Arbitrage System SHALL включить поле is_stale в WebSocket сообщения для фронтенда
3. WHEN состояние Stale, THE Dashboard SHALL отобразить красный индикатор предупреждения
4. THE Arbitrage System SHALL НЕ генерировать торговые сигналы при Stale состоянии
5. WHEN данные снова становятся актуальными, THE Arbitrage System SHALL автоматически снять флаг Stale

### Requirement 13

**User Story:** Как разработчик, я хочу иметь оптимизированную конфигурацию сборки, чтобы максимизировать производительность системы

#### Acceptance Criteria

1. THE Cargo.toml SHALL включать opt-level = 3 для release профиля
2. THE Cargo.toml SHALL включать lto = true для Link-Time Optimization
3. THE Cargo.toml SHALL включать codegen-units = 1 для максимальной оптимизации
4. THE Arbitrage System SHALL НЕ использовать println! в критических путях обработки данных
5. THE Arbitrage System SHALL использовать #[inline] атрибуты для hot path функций

### Requirement 14

**User Story:** Как DevOps инженер, я хочу иметь Docker конфигурацию для быстрого развертывания, чтобы упростить деплой системы

#### Acceptance Criteria

1. THE Project SHALL включать docker-compose.yml для одновременного запуска backend и frontend
2. THE Docker configuration SHALL использовать multi-stage build для минимизации размера образа
3. THE Docker configuration SHALL монтировать .env файл для конфигурации API ключей
4. THE Docker configuration SHALL экспонировать порт 3000 для WebSocket сервера
5. THE Docker configuration SHALL экспонировать порт 4200 для Angular development server

### Requirement 15

**User Story:** Как трейдер, я хочу видеть Bloomberg-style интерфейс, чтобы получить профессиональный опыт мониторинга

#### Acceptance Criteria

1. THE Dashboard SHALL использовать Tailwind CSS с цветовой схемой bg-slate-900 для фона
2. THE Dashboard SHALL использовать text-emerald-400 для положительных значений спреда
3. THE Dashboard SHALL использовать text-red-400 для отрицательных значений спреда
4. THE Dashboard SHALL отображать компактную таблицу цен с монопространственным шрифтом
5. THE Dashboard SHALL использовать Zoneless change detection для максимальной производительности
