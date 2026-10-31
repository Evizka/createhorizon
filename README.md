# Create → Horizon

Идиоматичный Rust-порт механик Create. Симуляционное ядро — `no_std`,
без `unsafe` и без floating-point в математике мира. В репозитории теперь есть
устанавливаемый smoke-test мод: native `.hnmod` с read-only HUD/командой и
отдельный content `.hmod` с шестью тестовыми блоками.

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
- Content `.hmod` добавляет `createhorizon:hand_crank`, `shaft`,
  `small_cogwheel`, `large_cogwheel`, `millstone` и `crushing_wheel`.
- Native `.hnmod` добавляет mod в Options > Mods, небольшой HUD и команду
  `/createhorizon scan`. Команда сканирует блоки в радиусе 8 и выводит числа
  сети и детерминированный state hash; HUD пересчитывает область радиусом 3.
- Тестовый hand crank — постоянный источник 16 RPM с capacity 16. Millstone
  имеет демонстрационный stress 12, crushing wheel — 24. Эти значения нужны
  для воспроизводимого теста, а не заявляют полную parity с Create.

Тестовая сборка не меняет мир напрямую: она только читает публичные block
states, вычисляет модель сети и показывает результат. Блоки для smoke test —
простые кубы с ванильными текстурами; нет вращающейся анимации, интерактивной
рукоятки и полного набора Create.

## Пакеты

Пакеты уже собраны в [`artifacts/`](artifacts/):

| Файл | Назначение | SHA-256 |
|---|---|---|
| `createhorizon-0.1.0.hmod` | Content blocks, recipes, loot и модели | `4a5c4f0c141a24a26fc473e85d6517a1915df49a8f0dc492864ee1dc79768c6b` |
| `createhorizon_native-0.1.0.hnmod` | Native Horizon Mixins source bundle | `7ec7884362c2ad134f4f686910ccdab9cd040d81f9b0118d197d3a5ea387cd84` |

Пакеты можно воспроизводимо пересобрать и проверить локально:

```sh
python3 tools/package.py
```

`horizon-mixin.toml` и `compatibility.json` pin'ят Horizon
`ba6344a59cb9e755b730dcf02285fc0d33b97d3c`, horizon-mixins
`d9c83d7096b7ea5b83678da2efdfd6161f74997e`, game line `26.3` и native hooks
API `11`. SHA-256 целевых Horizon-файлов проверяется при staging.

## Быстрая проверка в игре

Нужен Horizon, чей launcher принимает native `.hnmod` этого pinned build, и
подготовленные ресурсы игры 26.3. В каталоге игры (где `options.txt`) положите:

```text
horizon-mods/createhorizon-0.1.0.hmod
horizon-native-mods/createhorizon_native-0.1.0.hnmod
```

Запустите игру. В Options > Mods включите **Create Horizon Kinetics** и
**Create Horizon 0.1.0**; после переключения content-мода перезапустите игру.
В creative inventory найдите тестовые блоки или скрафтите их. Постройте сеть
рядом с игроком, где координаты относительны к hand crank:

```text
hand_crank       (0, 0,  0)
shaft            (0, 0,  1)
large_cogwheel   (0, 0,  2)
small_cogwheel   (1, 1,  2)
millstone        (0, 0, -1)
```

Откройте чат и выполните `/createhorizon scan`. Для этой схемы ожидаются
`parts=5`, `links=4`, `networks=1`, `driven=5`, `max=32 rpm`, `overload=0`.
Затем замените millstone на `crushing_wheel`: ожидаются `overload=1`,
`driven=0`, `max=0 rpm`. HUD показывает ближайшую сеть, если она в радиусе 3.

Это end-to-end gameplay smoke test после сборки native bundle, не гарантия
совместимости с произвольной веткой Horizon. `.hnmod` содержит исходники, а не
готовый исполняемый файл; для его компиляции нужен launcher/build, совместимый
с указанным Horizon revision.

## Сборка native-варианта из исходников

Для воспроизводимого локального build понадобятся Linux, Rust 1.95.0, Git,
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

## Проверки этого checkout

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
python3 tools/package.py
```

В текущей sandbox-среде нет `rustc`, `cargo`, `rustfmt` и `clippy-driver`,
поэтому Rust-сборка и unit tests здесь не запускались. Выполнены упаковка и
проверки Python/TOML/JSON/Rust-синтаксиса; это не подменяет компиляцию Rust и
Horizon stage.

## Источники и ограничения

`create-rs` из handoff не найден в checkout, upload-папке или публичном
поиске. Поведение шестерён восстанавливается по Create upstream
`mc1.21.1/dev`, commit `4148fd8d5b05c85d88947ed00fb495a5074f57`; это reference,
не исходный Rust-проект. Правила fixed-point, версии и граница Horizon описаны
в [`docs/strict-math.md`](docs/strict-math.md).
