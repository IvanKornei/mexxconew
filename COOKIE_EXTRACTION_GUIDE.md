# Руководство по извлечению cookies для MEXC

## Обзор

Поскольку MEXC не предоставляет API для торговли с плечами, мы используем эмуляцию браузера. Есть два способа получить cookies для аутентификации:

### ✅ Рекомендуемый: Автоматическое извлечение из браузера

Самый простой и быстрый способ - автоматически извлечь cookies из вашего активного браузера.

### 🔧 Альтернатива: Ручной ввод cookies

Если автоматическое извлечение не работает, можно вручную скопировать cookies из DevTools.

---

## Метод 1: Автоматическое извлечение (Рекомендуется)

### Требования

1. Вы должны быть залогинены в MEXC в одном из браузеров:
   - Google Chrome
   - Microsoft Edge  
   - Mozilla Firefox

2. Посетите https://futures.mexc.com хотя бы один раз

### Шаги

1. **Закройте браузер** (для избежания блокировки базы данных)

2. **Запустите утилиту извлечения cookies:**

```bash
cargo run --bin extract_cookies
```

3. **Утилита автоматически:**
   - Определит ваш браузер
   - Извлечет все MEXC cookies
   - Покажет их в удобном формате

4. **Скопируйте вывод** - cookies будут показаны в двух форматах:
   - Строка для ручного ввода
   - JSON для программного использования

### Пример вывода

```
🍪 MEXC Cookie Extractor

🔍 Auto-detecting browser...
✅ Browser detected!

✅ Found 5 cookies:

  📌 session_id
     Domain: .mexc.com
     Value: eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
     Secure: true, HttpOnly: true
     Expires: 2026-03-08 12:00:00 UTC

  📌 auth_token
     Domain: futures.mexc.com
     Value: Bearer_abc123xyz789...
     Secure: true, HttpOnly: true

📋 Cookie string for manual input:
─────────────────────────────────────
session_id=eyJhbGc...; auth_token=Bearer_abc...
─────────────────────────────────────
```

---

## Метод 2: Ручное извлечение через DevTools

Если автоматическое извлечение не работает:

### Шаги

1. **Откройте MEXC Futures в браузере:**
   - Перейдите на https://futures.mexc.com
   - Убедитесь, что вы залогинены

2. **Откройте DevTools:**
   - Нажмите `F12` или `Ctrl+Shift+I`
   - Или правый клик -> "Inspect" -> "Application"

3. **Найдите cookies:**
   - В левой панели: `Application` -> `Storage` -> `Cookies`
   - Выберите `https://futures.mexc.com`

4. **Скопируйте важные cookies:**
   
   Нужны cookies с именами типа:
   - `session_id`
   - `auth_token` 
   - `token`
   - `_mexc_*`
   - Любые другие с длинными значениями

5. **Сформируйте строку:**

```
session_id=VALUE1; auth_token=VALUE2; token=VALUE3
```

---

## Использование в коде

### Вариант A: Автоматическое извлечение

```rust
use arbitrage_system::emulation::{
    session::SessionInitializer,
    config::EmulationConfig,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = EmulationConfig::default();
    let mut initializer = SessionInitializer::new(config)?;
    
    // Автоматически извлечь cookies из браузера
    let session = initializer.initialize_from_browser_cookies().await?;
    
    println!("✅ Session initialized with {} cookies", session.cookies.len());
    
    Ok(())
}
```

### Вариант B: Ручной ввод

```rust
use arbitrage_system::emulation::{
    session::SessionInitializer,
    config::EmulationConfig,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = EmulationConfig::default();
    let mut initializer = SessionInitializer::new(config)?;
    
    // Вставьте вашу строку cookies
    let cookie_string = "session_id=abc123; auth_token=xyz789; token=def456";
    
    let session = initializer
        .initialize_from_manual_cookies(cookie_string)
        .await?;
    
    println!("✅ Session initialized");
    
    Ok(())
}
```

### Вариант C: Из переменной окружения

Добавьте в `.env`:

```env
MEXC_COOKIES="session_id=abc123; auth_token=xyz789; token=def456"
```

Затем в коде:

```rust
let cookie_string = std::env::var("MEXC_COOKIES")?;
let session = initializer
    .initialize_from_manual_cookies(&cookie_string)
    .await?;
```

---

## Периодическое обновление cookies

Cookies имеют срок действия. Система автоматически обновляет их:

```rust
use std::time::Duration;
use tokio::time::interval;

// Обновлять каждые 30 минут
let mut refresh_interval = interval(Duration::from_secs(30 * 60));

loop {
    refresh_interval.tick().await;
    
    // Попытка обновить cookies из браузера
    match initializer.initialize_from_browser_cookies().await {
        Ok(new_session) => {
            println!("✅ Cookies refreshed");
            // Обновить активную сессию
        }
        Err(e) => {
            println!("⚠️  Failed to refresh cookies: {}", e);
            // Продолжить с текущими cookies
        }
    }
}
```

---

## Troubleshooting

### Проблема: "No supported browser found"

**Решение:**
- Установите Chrome, Edge или Firefox
- Убедитесь, что браузер установлен в стандартную директорию
- Попробуйте ручное извлечение

### Проблема: "No MEXC cookies found"

**Решение:**
- Убедитесь, что вы залогинены в MEXC
- Посетите https://futures.mexc.com
- Закройте браузер и попробуйте снова

### Проблема: "Failed to open cookie database"

**Решение:**
- Полностью закройте браузер (проверьте Task Manager)
- Запустите утилиту от имени администратора
- Попробуйте другой браузер

### Проблема: "Session expired" во время торговли

**Решение:**
- Включите автоматическое обновление cookies (см. выше)
- Держите браузер открытым с активной сессией MEXC
- Настройте более частое обновление

---

## Безопасность

### ⚠️ Важно

- **Никогда не делитесь** своими cookies - они дают полный доступ к аккаунту
- **Не коммитьте** cookies в Git
- **Используйте .env** для хранения cookies локально
- **Регулярно обновляйте** cookies для безопасности

### Рекомендации

1. Добавьте в `.gitignore`:
```
.env
.kiro/emulation/session.enc
cookies.txt
```

2. Используйте шифрование для хранения:
```bash
# Установите ключ шифрования
export EMULATION_SESSION_KEY="your-64-char-hex-key"
```

3. Ограничьте права доступа к файлам:
```bash
chmod 600 .env
chmod 600 .kiro/emulation/session.enc
```

---

## Альтернатива: Google OAuth (будущее)

В будущем можно добавить интеграцию с Google OAuth для автоматического логина:

```rust
// TODO: Implement when needed
pub async fn initialize_with_google_oauth(&mut self) -> Result<SessionData> {
    // 1. Open browser with Google OAuth flow
    // 2. User logs in with Google
    // 3. MEXC redirects back with session
    // 4. Extract cookies automatically
}
```

Это потребует:
- Интеграцию с `eoka` (browser automation)
- OAuth client credentials
- Headless browser setup

---

## Дополнительные ресурсы

- [EMULATION_SYSTEM.md](./EMULATION_SYSTEM.md) - Полная документация системы эмуляции
- [EMULATION_INTEGRATION_GUIDE.md](./EMULATION_INTEGRATION_GUIDE.md) - Интеграция с торговой системой
- [config.toml](./config.toml) - Конфигурация эмуляции

---

## Вопросы?

Если у вас возникли проблемы:

1. Проверьте логи: `RUST_LOG=debug cargo run --bin extract_cookies`
2. Попробуйте другой браузер
3. Используйте ручное извлечение как fallback
4. Проверьте, что MEXC не заблокировал ваш аккаунт
