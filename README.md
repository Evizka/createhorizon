# Create → Horizon

Идиоматичный Rust-порт механик Create для Horizon (Minecraft: Java Edition
26.3, переписанный на Rust). Симуляционное ядро — `no_std`, без `unsafe` и без
floating-point в математике мира. В репозитории есть устанавливаемый мод:
content `.hmod` с одиннадцатью блоками и native `.hnmod` с HUD, командой,
двумя клавишами и страницей настроек.

> **Это native Horizon mod, не WASM.** Проверенный Horizon `main` не исполняет
> игровые WASM-моды с кодом. `.hnmod` — source-only Rust mixin bundle, который
> launcher компилирует для Horizon; `.hmod` содержит только данные и ресурсы.
> Выбор native-пути здесь намеренный и обозначен в пакете и инструкциях.

## Что можно проверить

- `create-engine` — единый источник математики, геометрии шестерён и
  детерминированного kinetic solver. Он находится в
  `mixins/createhorizon_native/engine` и объявлен `no_std`.
- `create-math` и `create-kinetics` — тонкие совместимые re-export facades,
  чтобы Rust-потребители могли использовать небольшие crates без копий кода.
- Content `.hmod` добавляет одиннадцать блоков в неймспейсе `createhorizon:`
  (таблица ниже).
- Native `.hnmod` регистрируется в Options > Mods, добавляет HUD, команду
  `/createhorizon scan` и две настраиваемые клавиши: **C** включает ближайшую
  рукоятку, **V** размыкает и смыкает все сцепления.

### Блоки

Content-мод Horizon копирует ванильный блок-шаблон и не умеет объявлять
block-state properties, поэтому ось каждой детали фиксирована её id.

| Блок | Ось | Роль в сети |
|---|---|---|
| `hand_crank` | Z | Ручной источник: 16 RPM, capacity 16, включается клавишей |
| `shaft` | Z | Вал: передаёт вращение 1:1 вдоль своей оси |
| `vertical_shaft` | Y | То же, но по вертикали |
| `small_cogwheel` | Z | Малая шестерня (контакты по правилам Create) |
| `large_cogwheel` | Z | Большая шестерня |
| `millstone` | Z | Нагрузка: stress 12 |
| `crushing_wheel` | Z | Нагрузка: stress 24 |
| `water_wheel` | Z | Источник от воды вокруг: до 4 RPM и capacity 16 |
| `gearbox` | Z → Y | Редуктор: принимает вращение по Z и отдаёт его по Y, инвертированным |
| `speedometer` | любая | Спидометр: показывает скорость сети, не добавляя stress |
| `clutch` | любая | Сцепление: разомкнутое рвёт все свои контакты |

Стресс жернов и дробилки — демонстрационные параметры (12 и 24), а не
заявление о полной parity с Create.

### Правила новых деталей

- **Водяное колесо.** С четырёх сторон, перпендикулярных его оси, читается
  вода: каждая сторона даёт `level` (1..8, источник и падающий столб — 8) и
  ещё 4, если вода падает. Сумма ограничена полным потоком 16; скорость равна
  `flow / 4` RPM, capacity — `flow` единиц. Сухое колесо — пассивный вал.
- **Редуктор.** Вращение заходит на его входную грань (Z) и выходит с выходной
  (Y) инвертированным. По входной оси редуктор передаёт вращение и напрямую,
  сквозь себя.
- **Сцепление.** Разомкнутое, оно не проводит ни один контакт: сети по обе
  стороны работают порознь.
- **Спидометр и сцепление** — фитинги: они представляют грань на всех трёх
  осях, поэтому один блок подходит к линии о любом направлении. Спидометр не
  имеет ни источника, ни нагрузки; HUD и команда показывают скорость там, где
  он стоит.

Индикатор вращения в HUD — анимация 2D-индикатора, а не вращение моделей в
мире: закреплённый native hook API не даёт callback рендера блоков. Мод только
читает публичные block states и не меняет мир.

## Пакеты

В [`artifacts/`](artifacts/) лежат собранные пакеты:

| Файл | Назначение | SHA-256 |
|---|---|---|
| `createhorizon-0.1.0.hmod` | Content blocks, recipes, loot и модели | `8a0190d7fc2f96d2791f49959f326f3a3f7cd8804998709a1e429386c0cc7a3d` |
| `createhorizon_native-0.1.0.hnmod` | Native Horizon Mixins source bundle | `5a13aabce47c088bae3e65bbc4a2a58011e5735c16d385b624a4eab26629f22e` |

Хэши пересчитываются при каждой пересборке:

```sh
python3 tools/package.py
```

`horizon-mixin.toml` и `compatibility.json` pin'ят Horizon
`ba6344a59cb9e755b730dcf02285fc0d33b97d3c`, horizon-mixins
`d9c83d7096b7ea5b83678da2efdfd6161f74997e`, game line `26.3` и native hooks
API `3`. SHA-256 целевых Horizon-файлов проверяется при staging.

## Быстрая проверка в игре

Нужен Horizon, чей launcher принимает native `.hnmod` этого pinned build, и
подготовленные ресурсы игры 26.3. В каталоге игры (где `options.txt`) положите:

```text
horizon-mods/createhorizon-0.1.0.hmod
horizon-native-mods/createhorizon_native-0.1.0.hnmod
```

Запустите игру. В Options > Mods включите **Create Horizon Kinetics** и
**Create Horizon 0.1.0**; там же, на странице мода, лежат две кнопки —
состояние сцепления и рукоятки. После переключения content-мода перезапустите
игру. В creative inventory найдите блоки или скрафтите их.

**Сеть 1 — базовая.** Координаты относительны к рукоятке:

```text
hand_crank       (0, 0,  0)
shaft            (0, 0,  1)
large_cogwheel   (0, 0,  2)
small_cogwheel   (1, 1,  2)
millstone        (0, 0, -1)
```

Нажмите C один раз, чтобы включить ближайшую рукоятку. `/createhorizon scan`
должен сообщить `parts=5`, `links=4`, `networks=1`, `driven=5`, `max=32 rpm`,
`overload=0`. Замените millstone на `crushing_wheel`: при включённом приводе
ожидаются `overload=1`, `driven=0`, `max=0 rpm`.

**Сеть 2 — вода, редуктор, спидометр, сцепление:**

```text
water_wheel      (0, 0, 0)   с водой в (1,0,0), (-1,0,0), (0,1,0)
gearbox          (0, 0, 1)
vertical_shaft   (0, 1, 1)
speedometer      (0, 2, 1)
clutch           (0, 3, 1)
vertical_shaft   (0, 4, 1)
```

Колесо гонит `flow=16`, `max=4 rpm`; спидометр на выходе редуктора показывает
`gauge=-4 rpm` (выходная грань инвертирована) при `parts=6`, `links=5`,
`networks=1`, `driven=6`. Нажмите V — сцепление разомкнётся и рвёт оба своих
контакта: `networks=3`, `links=3`, `driven=4`, `clutch=released`; вал за
сцеплением остановится. Нажмите V ещё раз, чтобы сомкнуть.

Это end-to-end gameplay smoke test после пересборки native bundle, не гарантия
совместимости с произвольной веткой Horizon. `.hnmod` содержит исходники, а не
готовый исполняемый файл; для его компиляции нужен launcher/build, совместимый
с указанным Horizon revision.

## Проверки этого checkout

```sh
python3 tools/bootstrap-rust.py                 # ставит Rust 1.97.0, печатает путь
export PATH="$(python3 tools/bootstrap-rust.py):$PATH"
cargo fmt --all --check
cargo test --workspace --lib --tests
cargo clippy --workspace --all-targets -- -D warnings
python3 tools/package.py
```

`cargo test --workspace` (без `--lib --tests`) дополнительно прогоняет doctests и
требует `rustdoc`, которого в офлайн-тулчейне нет; в обычной установке Rust он
есть.

Кроме engine-тестов, `harness/` собирает стенд: он воспроизводит типы хоста,
которые называет бандл (`Game`, `HudBatch`, `Corners`, `NativeMod`, мир и
state facts, а также `horizon_client::blocks::Fluid`, который читает колесо),
и `include!`-ит исходники бандла. Так миксины компилируются и тестируются
здесь, без Horizon checkout:

```sh
cargo test -p createhorizon-harness
```

Это проверка, а не порт: стенд моделирует закреплённый API `horizon-client`,
и все значения в его тестах — фикстуры самого мода.

## Сборка native-варианта из исходников

Для воспроизводимого локального build понадобятся Linux, Rust 1.95.0+, Git,
Cargo, Python 3.11+ и исходники с точными pin'ами. Команды собирают отдельный
stage и не меняют исходный Horizon checkout:

```sh
git clone https://github.com/horizon-rust/horizon ../horizon
git -C ../horizon checkout ba6344a59cb9e755b730dcf02285fc0d33b97d3c
git clone https://github.com/horizon-rust/horizon-mixins ../horizon-mixins
git -C ../horizon-mixins checkout d9c83d7096b7ea5b83678da2efdfd6161f74997e
cargo fetch --locked --manifest-path ../horizon-mixins/Cargo.toml
cargo fetch --locked --manifest-path ../horizon/Cargo.toml
cargo run --locked --manifest-path ../horizon/Cargo.toml -p cargo-horizon-mod -- check artifacts/createhorizon-0.1.0.hmod
cargo run --locked --manifest-path ../horizon/Cargo.toml -p cargo-horizon-mod -- check artifacts/createhorizon_native-0.1.0.hnmod
python3 tools/horizon.py stage --horizon ../horizon
cargo fetch --locked --manifest-path target/mixins/modded/Cargo.toml
python3 tools/horizon.py test --horizon ../horizon
python3 tools/horizon.py build --horizon ../horizon
```

Построенный клиент по умолчанию будет в
`target/horizon-cargo/fast/horizon-client`; для запуска передайте ему путь к
подготовленным ресурсам Horizon. `tools/horizon.py check` выполняет только
compile check, `test` также запускает две regression-тесты интегрированного
sample solver, `build` создаёт native client. Для повторного использования
другого каталога Cargo передайте `--target-dir PATH`.

## Источники и ограничения

`create-rs` из handoff не найден в checkout, upload-папке или публичном
поиске. Поведение шестерён восстанавливается по Create upstream
`mc1.21.1/dev`, commit `4148fd8d5b05c85d88947ed00fb495a5074f57`; это reference,
не исходный Rust-проект. Правила fixed-point, версии и граница Horizon описаны
в [`docs/strict-math.md`](docs/strict-math.md).
