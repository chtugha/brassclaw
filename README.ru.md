<p align="center">
  <img src="brassclaw.png?v=2" alt="BrassClaw" width="200"/>
</p>

<h1 align="center">BrassClaw</h1>

<p align="center">
  <strong>Ваш защищённый персональный AI-ассистент — работает полностью на вашем железе</strong>
</p>

<p align="center">
  <a href="https://github.com/chtugha/brassclaw/releases/latest"><img src="https://img.shields.io/github/v/release/chtugha/brassclaw?label=последний%20релиз" alt="Latest Release" /></a>
  <a href="#license"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache%202.0-blue.svg" alt="Лицензия: MIT OR Apache-2.0" /></a>
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-CN.md">简体中文</a> |
  <a href="README.ru.md">Русский</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.ko.md">한국어</a>
</p>

<p align="center">
  <a href="#философия">Философия</a> •
  <a href="#возможности">Возможности</a> •
  <a href="#быстрый-старт">Быстрый старт</a> •
  <a href="#конфигурация">Конфигурация</a> •
  <a href="#архитектура">Архитектура</a>
</p>

---

## Философия

BrassClaw построен на простом принципе: **ваш AI-ассистент должен работать на вас.**

- **100% локальная работа** — запускается с vLLM, Ollama или любым OpenAI-совместимым сервером; облачный аккаунт не нужен
- **Ваши данные — ваши** — всё состояние хранится в локальной встроенной Postgres, зашифровано и не покидает ваш контроль
- **Поддержка потребительского железа** — 7B-модели работают при 4 GB VRAM
- **Эшелонированная защита** — процессная песочница, лизинг возможностей, фреймворк хуков, защита от инъекций промптов
- **Открытый исходный код** — полностью проверяемый, без телеметрии и сбора данных
- **Оркестратор прежде всего** — Monty-оркестратор является исполняющим движком; LLM задействуется только при необходимости

---

## Возможности

### Оркестратор-первый движок

- **Monty (Python-оркестратор)** последовательно выполняет шаги рецептов и напрямую вызывает инструменты Rust
- **Рецепты Tier-0** — полностью детерминированные пути выполнения без вызовов LLM
- **Навыки (Skills)** — markdown-файлы обучают систему использовать API; компиляция Rust не требуется
- **Петля проверки Sempai/Kohai** — автоматически создаёт новые компоненты (рецепты, навыки, ToolSkill) и ставит их в очередь валидации

### Безопасность прежде всего

- **Процессная песочница** — ненадёжные инструменты выполняются с ограниченными файловыми системами и списками разрешённых эндпоинтов
- **Фреймворк хуков** — 4 уровня доверия (Builtin, Trusted, Installed, SelfAuthored) для шлюзов вызовов возможностей и мутаций промптов
- **Лизинг возможностей** — детализированные, отзываемые гранты прав для каждого вызова инструмента
- **Защита учётных данных** — секреты никогда не передаются инструментам; внедряются на хост-границе с обнаружением утечек
- **Защита от инъекций промптов** — обнаружение паттернов, санитизация контента, применение политик
- **Список разрешённых эндпоинтов** — HTTP-запросы только к явно одобренным хостам и путям

### Всегда доступен

- **Мультиканальность** — REPL, WebUI (React SPA по пути `/v2`), Slack, Telegram, HTTP-вебхуки, API-сервер
- **Постоянная память** — гибридный полнотекстовый + векторный поиск с Reciprocal Rank Fusion на базе встроенной Postgres
- **Рутины** — cron-расписания, триггеры событий, обработчики вебхуков для фоновой автоматизации
- **Субагенты** — порождение специализированных дочерних агентов для сложных задач
- **Протокол MCP** — подключение к любому серверу Model Context Protocol

### Пайплайн валидации

Все компоненты, созданные пользователем или агентом, проходят двухэтапный пайплайн валидации:

- **Q1** — автоматизированная проверка в оркестрированной песочнице
- **Q2** — проверка человеком (только оператор; никогда не автоматизируется)

---

## Быстрый старт

**Минимальные требования:** ~8 GB RAM, современный 64-разрядный CPU, ~4 GB свободного места на диске.

### Вариант А: Linux-сервер — однострочная установка (рекомендуется)

Загружает последний готовый бинарник с GitHub Releases и регистрирует его как systemd-сервис:

```bash
curl -fsSL https://raw.githubusercontent.com/chtugha/brassclaw/main/install.sh | sudo bash
```

Установка конкретной версии:

```bash
sudo bash install.sh -v 0.9.1
```

**Удаление:**

```bash
curl -fsSL https://raw.githubusercontent.com/chtugha/brassclaw/main/uninstall.sh | sudo bash
```

### Вариант Б: macOS — ручная установка бинарника

```bash
# Apple Silicon (M1/M2/M3):
curl -fsSL -o brassclaw-reborn https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-macos-arm64
chmod +x brassclaw-reborn && sudo mv brassclaw-reborn /usr/local/bin/brassclaw-reborn

# Intel Mac:
curl -fsSL -o brassclaw-reborn https://github.com/chtugha/brassclaw/releases/latest/download/brassclaw-macos-amd64
chmod +x brassclaw-reborn && sudo mv brassclaw-reborn /usr/local/bin/brassclaw-reborn
```

### Вариант В: Сборка из исходников

Требуется [Rust 1.94+](https://rustup.rs).

```bash
git clone https://github.com/chtugha/brassclaw.git
cd brassclaw
cargo build --release --bin brassclaw
```

Бинарник находится в `target/release/brassclaw`.

---

## Конфигурация

### Файл конфигурации

Основная конфигурация находится в `~/.brassclaw/reborn/config.toml`. При первом запуске создаётся пример.

```toml
[llm.default]
provider_id = "openai_compatible"
model = "Qwen/Qwen2.5-7B-Instruct-AWQ"
base_url = "http://localhost:8000/v1"

[identity]
tenant = "my-instance"
```

### Переменные окружения

| Переменная | Описание |
|------------|----------|
| `BRASSCLAW_REBORN_HOME` | Каталог данных (по умолчанию: `~/.brassclaw/reborn`) |
| `BRASSCLAW_RUNTIME_PROFILE` | Политика безопасности (по умолчанию: `local_dev`). Допустимые значения: `local_dev`, `local_safe`, `local_yolo`, `hosted_safe` |
| `BRASSCLAW_PG_URL` | URL внешней Postgres. Если не задано — используется встроенная Postgres |
| `BRASSCLAW_REBORN_WEBUI_TOKEN` | Bearer-токен для аутентификации WebUI |
| `BRASSCLAW_REBORN_WEBUI_USER_ID` | Идентификатор пользователя, внедряемый в сессии |

---

## Архитектура

BrassClaw использует трёхуровневую модель:

- **Продукты** — владеют UX: CLI, WebUI (React SPA по пути `/v2`), Slack, Telegram
- **Циклы** — владеют поведением агента: Monty-оркестратор последовательно выполняет шаги рецептов и вызывает Rust-инструменты по имени
- **Ядро** — владеет авторитетом: абстракция LLM-провайдера, изолированное выполнение подпроцессов, внедрение учётных данных, применение политики безопасности

**Каталог компонентов** хранится в Postgres по кодам классов:

| Класс | Тип | Описание |
|-------|-----|----------|
| 1–3 | Skill | Описание паттерна задачи для оркестратора |
| 13 | ToolSkill | Дескриптор привязки — схема параметров, предусловия |
| 21 | Recipe | Полный скрипт хода: RecipeVariant, примеры интентов, ссылки на шаги |
| 22 | PythonCode | Исполнитель — вызывает `host.<tool>(...)` для диспетчеризации Rust |
| 23 | ExtensionCatalogue | Обзор домена |

---

## Лицензия

Лицензировано по одной из следующих лицензий на ваш выбор:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))
