/// Структурированное логирование с контекстом для HFT системы
use tracing::Level;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

/// Инициализирует систему логирования
pub fn init_logging(log_level: Level) -> anyhow::Result<()> {
    // Создаём фильтр на основе уровня логирования
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("{}", log_level)));
    
    // Компактный формат для production
    let fmt_layer = fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .compact()
        .with_filter(filter);
    
    tracing_subscriber::registry()
        .with(fmt_layer)
        .init();
    
    Ok(())
}

/// Инициализирует детальное логирование для разработки
pub fn init_detailed_logging() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("debug"));
    
    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .with_span_events(FmtSpan::CLOSE)
        .pretty()
        .with_filter(filter);
    
    tracing_subscriber::registry()
        .with(fmt_layer)
        .init();
    
    Ok(())
}

/// Инициализирует JSON логирование для production мониторинга
pub fn init_json_logging(log_level: Level) -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("{}", log_level)));
    
    let fmt_layer = fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(false)
        .with_filter(filter);
    
    tracing_subscriber::registry()
        .with(fmt_layer)
        .init();
    
    Ok(())
}

#[macro_export]
macro_rules! log_trade {
    ($side:expr, $entry:expr, $exit:expr, $pnl:expr, $pnl_pct:expr) => {
        tracing::info!(
            side = ?$side,
            entry_price = $entry,
            exit_price = $exit,
            pnl = $pnl,
            pnl_percent = $pnl_pct,
            "Trade executed"
        );
    };
}

#[macro_export]
macro_rules! log_performance {
    ($operation:expr, $latency_us:expr) => {
        tracing::debug!(
            operation = $operation,
            latency_us = $latency_us,
            "Performance metric"
        );
    };
}
