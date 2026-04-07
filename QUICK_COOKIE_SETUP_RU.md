# Быстрая настройка cookies для MEXC

## 🚀 Самый простой способ (2 минуты)

### Шаг 1: Залогиньтесь в MEXC

1. Откройте Chrome/Edge/Firefox
2. Перейдите на https://futures.mexc.com
3. Войдите в свой аккаунт
4. **Закройте браузер полностью**

### Шаг 2: Извлеките cookies

```bash
cargo run --bin extract_cookies
```

Утилита автоматически найдет и извлечет ваши cookies.

### Шаг 3: Готово!

Cookies сохранены и готовы к использованию. Система будет автоматически обновлять их каждые 30 минут.

---

## 🔧 Альтернатива: Ручной ввод

Если автоматическое извлечение не работает:

### Шаг 1: Откройте DevTools

1. Откройте https://futures.mexc.com в браузере
2. Нажмите `F12`
3. Перейдите: `Application` → `Cookies` → `futures.mexc.com`

### Шаг 2: Скопируйте cookies

Найдите и скопируйте значения cookies с именами:
- `session_id`
- `auth_token`
- `token`
- Любые другие с длинными значениями

### Шаг 3: Добавьте в .env

```env
MEXC_COOKIES="session_id=ВАШ_ЗНАЧЕНИЕ; auth_token=ВАШ_ЗНАЧЕНИЕ; token=ВАШ_ЗНАЧЕНИЕ"
```

### Шаг 4: Готово!

Система будет использовать эти cookies для аутентификации.

---

## 📊 Проверка работы

Запустите пример:

```bash
cargo run --example cookie_session_example
```

Вы должны увидеть:

```
✅ Session initialized successfully!
   Cookies: 5
   Valid until: 2026-03-08 12:00:00 UTC
```

---

## ⚙️ Настройка автообновления

В `config.toml`:

```toml
[emulation.cookies]
auto_extract_on_startup = true
auto_refresh_interval_minutes = 30  # Обновлять каждые 30 минут
preferred_browser = "Auto"  # Или "Chrome", "Edge", "Firefox"
```

---

## ❓ Проблемы?

### "No supported browser found"
→ Установите Chrome, Edge или Firefox

### "No MEXC cookies found"
→ Убедитесь, что вы залогинены и посетили futures.mexc.com

### "Failed to open cookie database"
→ Закройте браузер полностью (проверьте Task Manager)

### "Session expired"
→ Включите автообновление или перезапустите извлечение cookies

---

## 🔒 Безопасность

⚠️ **ВАЖНО:**
- Никогда не делитесь cookies - они дают полный доступ к аккаунту
- Не коммитьте `.env` в Git
- Используйте шифрование для хранения

Добавьте в `.gitignore`:
```
.env
.kiro/emulation/session.enc
```

---

## 📚 Подробная документация

- [COOKIE_EXTRACTION_GUIDE.md](./COOKIE_EXTRACTION_GUIDE.md) - Полное руководство
- [EMULATION_SYSTEM.md](./EMULATION_SYSTEM.md) - Система эмуляции
- [config.toml](./config.toml) - Конфигурация

---

## 💡 Совет

Держите браузер с активной сессией MEXC открытым в фоне - система будет автоматически синхронизировать cookies и сессия никогда не истечет!
