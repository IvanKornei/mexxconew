# Implementation Plan

## Overview

Этот документ описывает пошаговый план реализации HFT системы мониторинга и торговли арбитражем между Binance Futures и MEXC Futures. План разбит на фазы с четкими задачами и критериями завершения.

## Phase 1: Project Setup & Infrastructure

### Task 1.1: Initialize Rust Project

**Objective**: Создать базовую структуру Rust проекта с оптимизированной конфигурацией

**Steps**:
1. Создать новый Rust проект: `cargo new arbitrage-system --bin`
2. Настроить Cargo.toml с оптимизациями для HFT
3. Создать модульную структуру директорий
4. Настроить .gitignore

**Deliverables**:
- `Cargo.toml` с dependencies и release profile
- Структура директорий: `src/exchanges/`, `src/core/`, `src/api/`, `src/utils/`
- `.gitignore` с исключением `.env`, `target/`, `logs/`

**Acceptance Criteria**:
- `cargo build --release` успешно компилируется
- Release profile включает `opt-level = 3`, `lto = true`, `codegen-units = 1`

### Task 1.2: Initialize Angular Project

**Objective**: Создать Angular 21 проект с Tailwind CSS и Signals

**Steps**:
1. Создать Angular проект: `ng new frontend --standalone --routing=false`
2. Установить и настроить Tailwind CSS 4.0+
3. Настроить Zoneless change detection
4. Создать базовую структуру компонентов

**Deliverables**:
- Angular проект в директории `frontend/`
- `tailwind.config.js` с темной темой (slate-900, emerald-400, red-400)
- Структура: `services/`, `components/price-card/`, `components/spread-visualizer/`

**Acceptance Criteria**:
- `npm start` запускает dev server на порту 4200
- Tailwind CSS работает с темной темой


### Task 1.3: Configuration Files

**Objective**: Создать конфигурационные файлы для системы

**Steps**:
1. Создать `.env.example` с шаблоном для API ключей
2. Создать `config.toml` с настройками системы
3. Создать `docker-compose.yml` для контейнеризации

**Deliverables**:
- `.env.example` с `MEXC_API_KEY` и `MEXC_API_SECRET`
- `config.toml` с endpoints, fees, thresholds
- `docker-compose.yml` для backend и frontend

**Acceptance Criteria**:
- Конфигурация загружается без ошибок
- Docker compose успешно поднимает оба сервиса

## Phase 2: Backend Core - Data Ingestion Layer

### Task 2.1: Error Types & Utilities

**Objective**: Реализовать базовые типы ошибок и утилиты

**Steps**:
1. Создать `src/utils/errors.rs` с иерархией ошибок
2. Создать `src/utils/config.rs` для загрузки конфигурации
3. Настроить `tracing` для структурированного логирования

**Deliverables**:
- `Error`, `ConnectionError`, `ApiError` типы с `thiserror`
- `Config` struct с методом `load_from_file()`
- Инициализация `tracing_subscriber` в main.rs

**Acceptance Criteria**:
- Все ошибки имеют понятные сообщения
- Логи пишутся в JSON формате
- Конфигурация валидируется при загрузке

### Task 2.2: Exchange Connectors

**Objective**: Реализовать WebSocket подключения к биржам

**Steps**:
1. Создать `src/exchanges/client.rs` с trait `ExchangeConnector`
2. Реализовать `src/exchanges/binance.rs` для Binance Futures
3. Реализовать `src/exchanges/mexc.rs` для MEXC Futures
4. Добавить exponential backoff для reconnection

**Deliverables**:
- `ExchangeConnector` trait с методами `connect()`, `receive()`, `reconnect_with_backoff()`
- `BinanceFuturesConnector` с подключением к `wss://fstream.binance.com/ws/btcusdt@aggTrade`
- `MexcFuturesConnector` с подключением к `wss://wbs.mexc.com/ws/public/v1/market/deal?symbol=BTC_USDT`
- Exponential backoff с максимум 5 попытками

**Acceptance Criteria**:
- WebSocket соединения устанавливаются успешно
- Автоматическое переподключение работает при обрыве
- Сообщения парсятся с использованием serde


### Task 2.3: Price Feed Manager

**Objective**: Создать менеджер для параллельной обработки данных от бирж

**Steps**:
1. Создать `src/core/price_feed.rs` с `PriceFeedManager`
2. Реализовать параллельные async tasks для каждой биржи
3. Добавить нормализацию данных
4. Реализовать latency tracking

**Deliverables**:
- `PriceFeedManager` с методом `start()`
- Отдельные tasks для Binance и MEXC
- Вычисление разницы между системным временем и timestamp биржи
- Нормализация цен к f64

**Acceptance Criteria**:
- Оба WebSocket работают параллельно
- Latency tracking показывает реальные значения
- Данные нормализуются корректно

### Task 2.4: Shared State & Stale Detection

**Objective**: Реализовать shared state с обнаружением устаревших данных

**Steps**:
1. Создать `src/core/state.rs` с `PriceState`
2. Использовать `tokio::sync::watch` для broadcast
3. Реализовать stale detection (300ms timeout)
4. Добавить поле `is_stale` в состояние

**Deliverables**:
- `PriceState` struct с полями: `binance_price`, `mexc_price`, `spread_percent`, `is_stale`, `latency_ms`
- `watch::Sender` и `watch::Receiver` для broadcast
- Логика проверки последнего обновления

**Acceptance Criteria**:
- State обновляется без блокировок
- Stale flag устанавливается через 300ms
- Множественные consumers могут читать state

## Phase 3: Backend Core - Processing Layer

### Task 3.1: Spread Calculator

**Objective**: Реализовать расчет спреда с учетом комиссий

**Steps**:
1. Создать `src/core/spread.rs` с `SpreadCalculator`
2. Реализовать формулу: `((mexc - binance) / binance) * 100`
3. Добавить учет комиссий бирж
4. Пометить функции как `#[inline]` для оптимизации

**Deliverables**:
- `SpreadCalculator` с методами `calculate_raw_spread()` и `calculate_net_spread()`
- Конфигурируемые комиссии (Binance: 0.04%, MEXC: 0.02%)
- Метод `is_opportunity()` с threshold 0.3%

**Acceptance Criteria**:
- Расчет спреда занимает <10μs
- Комиссии учитываются корректно
- Inline оптимизация применена

### Task 3.2: Symbol Normalizer

**Objective**: Нормализация символов между биржами

**Steps**:
1. Создать `src/core/normalizer.rs` с `SymbolNormalizer`
2. Реализовать конвертацию BTCUSDT ↔ BTC_USDT ↔ BTC/USDT
3. Использовать zero-allocation где возможно

**Deliverables**:
- Методы `normalize_binance()`, `normalize_mexc()`, `to_canonical()`
- Canonical format: `BTC/USDT`

**Acceptance Criteria**:
- Все форматы корректно конвертируются
- Нет лишних аллокаций

## Phase 4: Backend - Communication Layer

### Task 4.1: Axum WebSocket Server

**Objective**: Создать WebSocket сервер для фронтенда

**Steps**:
1. Создать `src/api/websocket.rs` с Axum router
2. Реализовать WebSocket handler
3. Broadcast price updates всем подключенным клиентам
4. Добавить health check endpoint

**Deliverables**:
- Axum server на порту 3000
- WebSocket endpoint `/ws`
- Health check endpoint `/health`
- JSON сообщения: `{ binance: f64, mexc: f64, spread: f64, is_stale: bool, latency_ms: u64 }`

**Acceptance Criteria**:
- WebSocket сервер принимает множественные подключения
- Данные транслируются в реальном времени
- Health check возвращает 200 OK

### Task 4.2: Main Entry Point

**Objective**: Связать все компоненты в main.rs

**Steps**:
1. Настроить Tokio runtime
2. Инициализировать tracing
3. Запустить Price Feed Manager
4. Запустить Axum server
5. Обработать graceful shutdown

**Deliverables**:
- `src/main.rs` с полной инициализацией
- Graceful shutdown при Ctrl+C
- Структурированное логирование

**Acceptance Criteria**:
- `cargo run --release` запускает систему
- Все компоненты работают параллельно
- Логи пишутся корректно

## Phase 5: Trading Infrastructure (Boilerplate)

### Task 5.1: ExchangeClient Trait

**Objective**: Создать trait для взаимодействия с биржами

**Steps**:
1. Создать `src/exchanges/client.rs` с trait `ExchangeClient`
2. Определить методы `place_order()`, `cancel_order()`, `get_balance()`
3. Использовать `async_trait` для async methods

**Deliverables**:
- `ExchangeClient` trait
- Типы: `OrderRequest`, `Order`, `Balance`, `OrderSide`, `OrderType`, `OrderStatus`

**Acceptance Criteria**:
- Trait компилируется без ошибок
- Все методы асинхронные

### Task 5.2: MEXC Client Implementation

**Objective**: Реализовать MEXC клиент с аутентификацией

**Steps**:
1. Обновить `src/exchanges/mexc.rs` с `MexcClient`
2. Реализовать HMAC-SHA256 подпись
3. Добавить методы `from_env()` для загрузки ключей
4. Создать boilerplate для `place_order()`, `cancel_order()`, `get_balance()`

**Deliverables**:
- `MexcClient` struct
- Метод `sign_request()` с HMAC-SHA256
- Загрузка API ключей из `.env`
- TODO комментарии для будущей реализации

**Acceptance Criteria**:
- Подпись генерируется корректно
- API ключи загружаются из `.env`
- Структура готова к реализации

## Phase 6: Angular Frontend

### Task 6.1: Market Data Service

**Objective**: Создать сервис для WebSocket подключения

**Steps**:
1. Создать `frontend/src/app/services/market-data.service.ts`
2. Реализовать WebSocket клиент
3. Использовать RxJS Subject для потока данных
4. Конвертировать в Signal через `toSignal()`

**Deliverables**:
- `MarketDataService` с WebSocket подключением к `ws://localhost:3000/ws`
- Signal для reactive state
- Auto-reconnect при обрыве

**Acceptance Criteria**:
- WebSocket подключается успешно
- Данные обновляются в реальном времени
- Signal работает корректно

### Task 6.2: Price Card Component

**Objective**: Создать компонент для отображения цены биржи

**Steps**:
1. Создать `frontend/src/app/components/price-card/`
2. Использовать Signal для reactive updates
3. Применить Tailwind CSS стили (bg-slate-900, text-white)
4. Монопространственный шрифт для цен

**Deliverables**:
- `PriceCardComponent` с Input для exchange и price
- Template с Tailwind стилями
- Форматирование цены с 2 знаками после запятой

**Acceptance Criteria**:
- Компонент отображается корректно
- Цены обновляются в реальном времени
- Стили соответствуют Bloomberg-style

### Task 6.3: Spread Visualizer Component

**Objective**: Создать виджет для отображения спреда

**Steps**:
1. Создать `frontend/src/app/components/spread-visualizer/`
2. Использовать `computed()` для расчета цвета
3. Зеленый (text-emerald-400) если > 0.05%, иначе серый
4. Добавить индикатор latency

**Deliverables**:
- `SpreadVisualizerComponent` с Input для spread и latency
- Computed для динамического цвета
- Отображение latency в мс

**Acceptance Criteria**:
- Цвет меняется динамически
- Latency отображается корректно
- Stale индикатор работает

### Task 6.4: Main Dashboard Layout

**Objective**: Собрать все компоненты в единый дашборд

**Steps**:
1. Обновить `frontend/src/app/app.component.ts`
2. Использовать Zoneless change detection
3. Создать компактную таблицу в стиле Bloomberg
4. Добавить Spread Health виджет

**Deliverables**:
- Главный layout с grid/flex
- Интеграция всех компонентов
- Темная тема (bg-slate-900)

**Acceptance Criteria**:
- Дашборд отображается корректно
- Все данные обновляются в реальном времени
- UI responsive

## Phase 7: Deployment & Documentation

### Task 7.1: Docker Configuration

**Objective**: Создать Docker конфигурацию для развертывания

**Steps**:
1. Создать `Dockerfile` для Rust backend (multi-stage build)
2. Создать `Dockerfile` для Angular frontend
3. Создать `docker-compose.yml` для обоих сервисов
4. Настроить volume для `.env` файла

**Deliverables**:
- `Dockerfile` для backend (Alpine-based)
- `Dockerfile` для frontend (nginx)
- `docker-compose.yml` с портами 3000 и 4200
- Volume mount для `.env`

**Acceptance Criteria**:
- `docker-compose up` запускает оба сервиса
- Сервисы взаимодействуют корректно
- Совместимость с WSL Ubuntu

### Task 7.2: Documentation & Setup Instructions

**Objective**: Создать документацию для запуска

**Steps**:
1. Создать `README.md` с инструкциями
2. Документировать требования к системе
3. Добавить примеры конфигурации
4. Создать troubleshooting секцию

**Deliverables**:
- `README.md` с полными инструкциями
- Примеры `.env` и `config.toml`
- Команды для Ubuntu WSL

**Acceptance Criteria**:
- Документация понятна и полна
- Все команды работают на Ubuntu WSL
- Примеры конфигурации валидны

## Testing & Validation

### Performance Benchmarks

**Metrics to Validate**:
- JSON parsing latency < 100μs
- Spread calculation latency < 10μs
- End-to-end latency < 1ms
- WebSocket message throughput > 1000 msg/s

**Tools**:
- `criterion` для Rust benchmarks
- `flamegraph` для profiling
- Browser DevTools для frontend

### Integration Tests

**Scenarios**:
1. WebSocket connection and reconnection
2. Stale detection after 300ms
3. Spread calculation accuracy
4. Frontend updates in real-time

## Deployment Checklist

- [ ] Rust backend компилируется с release optimizations
- [ ] Angular frontend собирается без ошибок
- [ ] WebSocket соединения работают
- [ ] Stale detection функционирует
- [ ] Latency tracking показывает реальные значения
- [ ] Docker compose запускается на WSL
- [ ] Логи пишутся корректно
- [ ] API ключи загружаются из .env
- [ ] Health check endpoint работает
- [ ] Frontend отображает данные в реальном времени

## Next Steps After Implementation

1. Добавить unit tests для критических компонентов
2. Реализовать полную MEXC trading integration
3. Добавить Binance trading client
4. Реализовать risk management модуль
5. Добавить historical data storage
6. Создать alerting систему
7. Оптимизировать для production deployment
