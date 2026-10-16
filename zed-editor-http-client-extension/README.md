# HTTP Client for Zed

HTTP-клиент для редактора [Zed](https://zed.dev) в стиле встроенного HTTP Client из IntelliJ IDEA.
Пишете запросы в файлах `.http` / `.rest`, нажимаете ▶ рядом с запросом — ответ появляется в терминале Zed.

Формат файлов совместим с IntelliJ IDEA / WebStorm / PhpStorm и в большой степени с VS Code REST Client,
поэтому существующие `.http`-файлы и `http-client.env.json` можно использовать без изменений.

---

## Содержание

- [Возможности](#возможности)
- [Как это устроено](#как-это-устроено)
- [Требования](#требования)
- [Установка](#установка)
- [Быстрый старт](#быстрый-старт)
- [Запуск запросов](#запуск-запросов)
- [Синтаксис .http-файлов](#синтаксис-http-файлов)
- [Переменные](#переменные)
- [Окружения](#окружения)
- [Навигация по переменным](#навигация-по-переменным)
- [Цепочки запросов](#цепочки-запросов)
- [Тело запроса](#тело-запроса)
- [Работа с ответом](#работа-с-ответом)
- [CLI zed-http](#cli-zed-http)
- [Горячие клавиши](#горячие-клавиши)
- [Структура проекта](#структура-проекта)
- [Разработка](#разработка)
- [Ограничения](#ограничения)
- [Решение проблем](#решение-проблем)

---

## Возможности

- Язык **HTTP** для файлов `.http` и `.rest`:
  - подсветка методов, URL, заголовков, переменных `{{...}}`, версий HTTP, разделителей `###`;
  - подсветка JSON, XML и GraphQL внутри тела запроса;
  - outline-панель (`cmd+shift+o`) со списком запросов файла;
  - комментарии `#` и `//` (`cmd+/`), автозакрытие скобок и кавычек;
  - **переход к определению переменной** по `cmd+click` / `F12` и подсказка со значением при наведении.
- Кнопка **▶ у каждого запроса** и задачи Zed для запуска одного или всех запросов файла.
- Раннер **`zed-http`**:
  - любые HTTP-методы, заголовки, тело, многострочные URL;
  - переменные файла, окружения (`http-client.env.json` + приватный файл), `$shared`;
  - динамические переменные: `$uuid`, `$timestamp`, `$isoTimestamp`, `$randomInt`, `$random.*`, `$processEnv.*`;
  - именованные запросы и ссылки на тело, заголовки и статус предыдущих ответов;
  - тело из файла (`< ./file.json`), подстановка файлов внутрь тела (multipart);
  - сохранение ответа в файл (`>>` и `>>!`);
  - сокращённая запись Basic-авторизации;
  - отключение редиректов (`# @no-redirect`);
  - цветной вывод: статус, время, размер, заголовки, отформатированный JSON;
  - автосохранение каждого ответа в `.zed-http/` для просмотра в Zed.

## Как это устроено

API расширений Zed не позволяет добавлять собственные панели и окна, поэтому проект состоит из двух частей:

| Часть | Где | Что делает |
|---|---|---|
| Расширение Zed | `extension.toml`, `languages/http/`, `src/lib.rs` | Регистрирует язык HTTP, грамматику tree-sitter, подсветку, outline, кнопки ▶, задачи и запускает language server |
| CLI `zed-http` | `cli/` | Выполняет запросы (`zed-http run`) и работает как language server (`zed-http lsp`) для перехода к переменным и hover |

При нажатии ▶ Zed запускает задачу:

```sh
zed-http run "<путь к файлу>" --line <номер строки>
```

Раннер находит запрос, в который попадает строка, выполняет его и выводит результат во встроенном терминале.

При открытии `.http`-файла Zed запускает `zed-http lsp` — он отвечает на «перейти к определению» и hover.

## Требования

- Zed (актуальная версия с поддержкой задач и runnables);
- Rust toolchain (`cargo`) — для сборки раннера и dev-расширения;
- macOS или Linux (Windows должен работать, но не проверялся).

## Установка

### 1. Раннер

```sh
git clone <этот репозиторий>
cd zed-editor-http-client-extension
cargo install --path cli
```

Бинарь `zed-http` попадёт в `~/.cargo/bin`. Проверьте, что этот каталог есть в `PATH`:

```sh
zed-http --help
```

> Zed запускает задачи через вашу оболочку. Если команда не находится внутри Zed,
> убедитесь, что `~/.cargo/bin` добавлен в `PATH` в `~/.zshrc` / `~/.bashrc`, а не только в текущей сессии.

### 2. Расширение

1. Откройте Zed.
2. Command palette (`cmd+shift+p`) → **`zed: install dev extension`**.
3. Выберите корневую папку этого репозитория.

Zed скомпилирует WASM-часть расширения (нужен `rustup`; target `wasm32-wasip2` Zed установит сам)
и грамматику tree-sitter-http. После этого файлы `.http` и `.rest` открываются как язык **HTTP**,
а внизу появляется language server **zed-http**.

### Обновление

```sh
git pull
cargo install --path cli --force
```

В Zed: `zed: extensions` → у dev-расширения нажмите **Rebuild**.

## Быстрый старт

Создайте `api.http`:

```http
@baseUrl = https://httpbin.org

### Получить данные
GET {{baseUrl}}/get?client=zed
Accept: application/json

### Создать запись
POST {{baseUrl}}/post
Content-Type: application/json

{
  "id": "{{$uuid}}",
  "name": "Zed"
}
```

Нажмите ▶ слева от `GET` — в терминале появится:

```
GET https://httpbin.org/get?client=zed
Accept: application/json

HTTP/1.1 200 OK  519 ms · 304 B
content-type: application/json
...

{
  "args": { "client": "zed" },
  ...
}

saved: ./.zed-http/line-3.json
```

Более полный пример — в [`examples/requests.http`](examples/requests.http) и
[`examples/http-client.env.json`](examples/http-client.env.json).

## Запуск запросов

| Способ | Что выполняется |
|---|---|
| ▶ рядом с методом | Этот запрос |
| `task: spawn` → «HTTP: GET …» | Запрос под курсором |
| `task: spawn` → «HTTP: run all requests in …» | Все запросы файла по порядку |
| `task: rerun` | Повтор последнего запуска |
| Горячая клавиша (см. [ниже](#горячие-клавиши)) | Ближайший к курсору запрос |

Из терминала:

```sh
zed-http run api.http --line 7
zed-http run api.http --name login
zed-http run api.http --all --env prod
```

## Синтаксис .http-файлов

### Структура запроса

```http
### Необязательное название
# @name имяЗапроса
МЕТОД URL [HTTP/версия]
Заголовок: значение
Заголовок: значение

тело запроса
```

- Запросы разделяются строкой, начинающейся с `###`. Текст после `###` — название запроса.
- Метод необязателен: строка `https://example.com` означает `GET`.
- Поддерживаемые методы: `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, `OPTIONS`, `TRACE`, `CONNECT`.
- Если в URL нет схемы, подставляется `http://`.
- Версия (`HTTP/1.1`, `HTTP/2`) в конце строки допустима и игнорируется.
- Заголовки идут сразу после строки запроса, тело — после пустой строки.

### Многострочный URL

Строки продолжения начинаются с отступа и склеиваются без пробелов:

```http
GET https://api.example.com/search
    ?q=zed
    &limit=10
    &sort=desc
```

### Комментарии

```http
# комментарий
// тоже комментарий
```

Комментарии между заголовками пропускаются.

### Директивы

| Директива | Значение |
|---|---|
| `# @name login` | Имя запроса: для `--name` и ссылок на ответ |
| `# @no-redirect` | Не следовать редиректам 3xx |

## Переменные

Переменные подставляются через `{{имя}}` в URL, заголовках, теле и путях к файлам.
Пробелы внутри скобок допустимы: `{{ host }}`. Значения переменных тоже могут содержать переменные.

### Переменные файла

```http
@host = api.example.com
@baseUrl = https://{{host}}/v1

GET {{baseUrl}}/users
```

Переменные файла действуют на весь файл и **перекрывают** значения из окружения.

### Динамические переменные

| Переменная | Результат |
|---|---|
| `{{$uuid}}`, `{{$random.uuid}}` | UUID v4 |
| `{{$timestamp}}` | Unix-время в секундах |
| `{{$isoTimestamp}}` | Время UTC в ISO 8601, например `2026-10-16T11:01:53.123Z` |
| `{{$randomInt}}` | Целое число 0–999 |
| `{{$randomInt 1 100}}`, `{{$random.integer 1 100}}` | Целое число в диапазоне `[1, 100)` |
| `{{$random.float}}` | Дробное число от 0 до 1 |
| `{{$random.alphabetic 8}}` | 8 случайных букв |
| `{{$random.alphanumeric 12}}` | 12 случайных букв и цифр |
| `{{$random.hexadecimal 16}}` | 16 hex-символов |
| `{{$processEnv.TOKEN}}`, `{{$env.TOKEN}}` | Переменная окружения ОС `TOKEN` |

Каждая подстановка генерирует новое значение.

### Порядок поиска

1. Динамические переменные (`$...`).
2. Переменные файла (`@name = ...`).
3. Выбранное окружение (приватный файл поверх публичного).
4. Блок `$shared`.
5. Ссылки на сохранённые ответы (`name.response...`).

Неизвестная переменная — ошибка, запрос не отправляется.

## Окружения

Файлы ищутся в каталоге `.http`-файла, затем выше по дереву до первого найденного.

**`http-client.env.json`** — коммитится в репозиторий:

```json
{
  "$shared": {
    "apiVersion": "v1"
  },
  "dev": {
    "baseUrl": "http://localhost:8080"
  },
  "prod": {
    "baseUrl": "https://api.example.com"
  }
}
```

**`http-client.private.env.json`** — секреты, добавьте в `.gitignore`:

```json
{
  "dev":  { "token": "dev-secret" },
  "prod": { "token": "prod-secret" }
}
```

Значения из приватного файла объединяются с публичным и перекрывают его. `$shared` доступен во всех окружениях.
Числа и boolean преобразуются в строки.

### Выбор окружения

По приоритету:

1. Флаг `--env prod`.
2. Переменная `HTTP_CLIENT_ENV`.
3. `dev`, `local` или `development`, если есть.
4. Первое окружение в файле.

Выбранное окружение печатается первой строкой вывода. Чтобы задачи Zed использовали другое окружение,
задайте переменную в настройках (`zed: open settings`):

```json
{
  "terminal": {
    "env": { "HTTP_CLIENT_ENV": "prod" }
  }
}
```

Или для отдельного проекта — в `.zed/settings.json`.

Список окружений:

```sh
zed-http envs api.http
```

## Навигация по переменным

`cmd+click` (или `F12`, `editor: go to definition`) по любой `{{переменной}}`:

| Что под курсором | Куда переходит |
|---|---|
| `{{token}}`, объявленная в файле через `@token = ...` | На строку `@token` |
| `{{baseUrl}}` из окружения | На ключ `"baseUrl"` в `http-client.env.json` / `http-client.private.env.json` |
| `{{login.response.body.token}}` | На `# @name login` |
| `{{$uuid}}` и другие динамические | Никуда |

Для переменных окружения сначала показываются определения в **текущем окружении** и в `$shared`.
Если переменная есть и в публичном, и в приватном файле, Zed покажет оба места списком.
Если в текущем окружении её нет — показываются все окружения, где она объявлена.

Переменная файла перекрывает окружение, поэтому при совпадении имён переход идёт на `@var`.

При наведении на переменную показывается её итоговое значение (с подставленными вложенными переменными) и источник:

```
baseUrl · environment `dev`
https://httpbin.org
```

Для ссылок на ответы значение берётся из `.zed-http/responses.json`; если запрос ещё не выполнялся, hover об этом скажет.
Для динамических переменных показывается пример значения.

Текущее окружение для навигации определяется так же, как для запуска (`HTTP_CLIENT_ENV`, затем `dev`/`local`).
После смены `HTTP_CLIENT_ENV` перезапустите сервер: `editor: restart language server`.

## Цепочки запросов

Ответы именованных запросов сохраняются, и на них можно ссылаться в следующих запросах:

```http
### Логин
# @name login
POST {{baseUrl}}/auth/login
Content-Type: application/json

{ "user": "admin", "password": "{{password}}" }

### Профиль
GET {{baseUrl}}/me
Authorization: Bearer {{login.response.body.accessToken}}
```

| Выражение | Значение |
|---|---|
| `{{login.response.body}}` | Всё тело |
| `{{login.response.body.data.token}}` | Поле JSON по пути |
| `{{login.response.body.$.data.token}}` | То же, в стиле JSONPath |
| `{{login.response.body.items[0].id}}` | Элемент массива |
| `{{login.response.headers.Location}}` | Заголовок (регистр неважен) |
| `{{login.response.status}}` | Код статуса |

Ответы хранятся в `.zed-http/responses.json` рядом с `.http`-файлом и **переживают перезапуск**:
достаточно один раз выполнить `login`, после чего остальные запросы можно запускать по отдельности.
Если ссылка указывает на ещё не выполненный запрос, раннер сообщит об этом.

Запросы без `@name` сохраняются под ключом `line-<номер строки>`.

## Тело запроса

### Встроенное тело

Всё после пустой строки до следующего `###`. Пустые строки в конце обрезаются.

```http
POST {{baseUrl}}/items
Content-Type: application/json

{
  "title": "Новая задача",
  "createdAt": "{{$isoTimestamp}}"
}
```

### Тело из файла

```http
POST {{baseUrl}}/upload
Content-Type: application/json

< ./payloads/item.json
```

Путь считается от каталога `.http`-файла. Файл отправляется как есть (байты), переменные в нём не подставляются.

### Form URL-encoded

```http
POST {{baseUrl}}/login
Content-Type: application/x-www-form-urlencoded

user=admin&password={{password}}
```

### Multipart

Строки вида `< ./file` внутри тела заменяются содержимым файла:

```http
POST {{baseUrl}}/upload
Content-Type: multipart/form-data; boundary=WebAppBoundary

--WebAppBoundary
Content-Disposition: form-data; name="meta"
Content-Type: application/json

{ "name": "report" }
--WebAppBoundary
Content-Disposition: form-data; name="file"; filename="report.txt"

< ./report.txt
--WebAppBoundary--
```

Подставляются только текстовые файлы; для бинарных загрузок используйте тело целиком из файла.

### Авторизация

```http
GET {{baseUrl}}/secure
Authorization: Basic {{user}} {{password}}
```

`Basic user password` и `Basic user:password` автоматически кодируются в Base64.
Уже закодированное значение передаётся без изменений. `Bearer` и прочие схемы — как есть.

## Работа с ответом

### Вывод

- строка запроса и отправленные заголовки;
- версия HTTP, статус (зелёный 2xx, жёлтый 3xx, красный 4xx/5xx), время, размер;
- заголовки ответа;
- тело: JSON форматируется с отступами (`--raw` — без форматирования), текст выводится как есть,
  для бинарных данных — только размер и тип.

### Автосохранение

Каждый ответ пишется в `.zed-http/<имя или line-N>.<ext>` рядом с `.http`-файлом.
Расширение выбирается по `Content-Type`: `json`, `html`, `xml`, `yaml`, `png`, `jpg`, `txt`, `bin`.
Путь в терминале кликабельный (`cmd+click`) — ответ откроется в Zed с подсветкой и поиском.

Добавьте в `.gitignore`:

```gitignore
.zed-http/
http-client.private.env.json
```

### Перенаправление в файл

```http
GET {{baseUrl}}/report.csv

>> ./out/report.csv
```

| Запись | Поведение |
|---|---|
| `>> path` | Если файл есть, создаётся `report-1.csv`, `report-2.csv`, … |
| `>>! path` | Файл перезаписывается |

Каталоги создаются автоматически.

### Код возврата

| Код | Значение |
|---|---|
| `0` | Все запросы вернули статус < 400 |
| `1` | Хотя бы один запрос вернул 4xx/5xx или упал |
| `2` | Ошибка аргументов, файла, переменных или окружения |

Удобно для CI и скриптов: `zed-http run smoke.http --all --env staging`.

## CLI zed-http

```
zed-http run <file.http> [--line N | --name NAME | --all] [--env ENV] [--raw] [--timeout SECS]
zed-http list <file.http>
zed-http envs <file.http>
zed-http lsp
```

| Команда / флаг | Описание |
|---|---|
| `run` | Выполнить запросы |
| `list` | Показать запросы: строка, метод, URL, имя |
| `envs` | Показать доступные окружения |
| `lsp` | Language server по stdio (запускается Zed автоматически) |
| `-l`, `--line N` | Запрос, содержащий строку `N` (нумерация с 1) |
| `-n`, `--name NAME` | Запрос по `@name` или названию после `###` |
| `-a`, `--all` | Все запросы файла по порядку |
| `-e`, `--env ENV` | Окружение |
| `--raw` | Не форматировать JSON |
| `--timeout SECS` | Таймаут запроса, по умолчанию 60 с |

## Горячие клавиши

`zed: open keymap` и добавьте:

```json
[
  {
    "context": "Editor && extension == http",
    "bindings": {
      "cmd-enter": ["editor::SpawnNearestTask", { "reveal": "no_focus" }]
    }
  }
]
```

`cmd-enter` выполняет запрос под курсором, фокус остаётся в редакторе.

## Структура проекта

```
.
├── extension.toml              манифест расширения, грамматика, language server
├── Cargo.toml                  WASM-часть расширения
├── src/lib.rs                  запуск `zed-http lsp` из Zed
├── languages/http/
│   ├── config.toml             расширения файлов, комментарии, скобки
│   ├── highlights.scm          подсветка
│   ├── injections.scm          JSON / XML / GraphQL в теле
│   ├── outline.scm             список запросов в outline
│   ├── runnables.scm           кнопки ▶
│   └── tasks.json              задачи запуска
├── cli/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs             аргументы, выполнение запросов, вывод
│       ├── lsp.rs              language server: definition, hover
│       ├── parser.rs           разбор .http-файлов
│       └── vars.rs             переменные, окружения, ссылки на ответы
└── examples/
    ├── requests.http
    └── http-client.env.json
```

Грамматика: [rest-nvim/tree-sitter-http](https://github.com/rest-nvim/tree-sitter-http), коммит указан в `extension.toml`.

## Разработка

```sh
cd cli
cargo test
cargo run -- list ../examples/requests.http
cargo run -- run ../examples/requests.http --all
```

Проверить сборку WASM-части:

```sh
cargo build --release --target wasm32-wasip2
```

После изменения `src/lib.rs` или файлов в `languages/http/` нажмите **Rebuild** у dev-расширения в `zed: extensions`.
Логи загрузки расширения: `zed: open log`.

Чтобы обновить грамматику, поменяйте `commit` в `extension.toml` и проверьте, что узлы,
используемые в `.scm`-файлах (`request`, `method`, `target_url`, `header`, `variable`, …), не переименованы.

## Ограничения

- JavaScript-обработчики (`> {% ... %}`, `> ./handler.js`) и pre-request скрипты пропускаются.
  Для передачи данных между запросами используйте `{{name.response...}}`.
- Нет UI-переключателя окружений — только `HTTP_CLIENT_ENV` или `--env`.
- Ответ показывается в терминале, а не в отдельной панели (ограничение API расширений Zed).
- Нет поддержки WebSocket, gRPC, GraphQL-подписок и `HTTP/2`-специфики; GraphQL-запросы отправляйте как `POST` с JSON.
- Нет cookie-jar между запросами — передавайте `Cookie` вручную или через `{{name.response.headers.Set-Cookie}}`.
- Значения `$processEnv` берутся из окружения терминала Zed.

## Решение проблем

**Нет кнопок ▶.**
Проверьте, что внизу справа язык файла — **HTTP**. Если нет — переустановите расширение и посмотрите `zed: open log`.
Кнопки также требуют включённого `"gutter": { "runnables": true }` (по умолчанию включено).

**▶ есть, но задача «HTTP: …» не предлагается.**
Скопируйте `languages/http/tasks.json` в `.zed/tasks.json` проекта (или в глобальный `~/.config/zed/tasks.json`).

**`cmd+click` по переменной ничего не делает.**
Проверьте, что `zed-http` новой версии в `PATH` (`zed-http lsp` не должен ругаться на неизвестную команду),
затем `editor: restart language server`. Ошибки сервера видны в `dev: open language server logs`.
Без `zed-http` в `PATH` расширение не сможет запустить сервер — подсветка работает, переходы нет.

**`zed-http: command not found`.**
`~/.cargo/bin` не в `PATH` оболочки, которую запускает Zed. Добавьте его в `~/.zshrc` и перезапустите Zed,
либо укажите в `tasks.json` полный путь: `"command": "/Users/<you>/.cargo/bin/zed-http"`.

**`unknown variable {{baseUrl}}`.**
Не найден `http-client.env.json` или в выбранном окружении нет переменной. Проверьте `zed-http envs file.http`
и строку `environment: ...` в начале вывода.

**`no stored response for request 'login'`.**
Сначала выполните запрос с `# @name login`.

**Ошибки TLS для локальных серверов с самоподписанным сертификатом.**
Используются системные корневые сертификаты; добавьте свой CA в системное хранилище.
