# Zed Editor Extensions

Extensions for the [Zed](https://zed.dev) editor.

| Extension                                   | What it gives                                                                                   |
|---------------------------------------------|-------------------------------------------------------------------------------------------------|
| [HTTP Client](http-client-extension/README.md)   | An IntelliJ IDEA-style HTTP client: run requests from `.http` / `.rest` files from the gutter, switch environments, and generate requests from routes and HTTP calls in 48 frameworks. |
| [Farum Azula Themes](theme-extension/README.md) | Two dark themes: **Farum Azula Transparent** (a transparent window) and **Farum Azula Dark** (opaque, for full screen). |
| [Flutter Tools](flutter-extension/README.md) | Dart and Flutter as in VS Code: run and debug on the iOS Simulator, Android, web and desktop from Zed's run window, hot reload on save and hot restart, 144 snippets that add the imports they need. Includes Zed's Dart extension, which it replaces. |

## Installation

Clone the repository and run the installer from its root:

```sh
git clone https://github.com/farum-azula/zed_editor_extensions.git
cd zed_editor_extensions
./install.sh                                              # macOS, Linux
```

```powershell
powershell -ExecutionPolicy Bypass -File install.ps1      # Windows
```

It installs every extension of the repository that Zed does not have yet, the same way as
`zed: install dev extension`: builds what an extension needs, then links its folder into Zed's
extensions. A running Zed picks the extensions up right away, without a restart. Run it again after
a `git pull`, and extensions added since then are installed too; the ones already installed are
left alone.

Needed for the build:

- [Rust](https://rustup.rs) installed with `rustup` (the installer adds the `wasm32-wasip2` target);
- `git`;
- an internet connection the first time: the grammars' sources and the wasi-sdk compiler Zed uses
  are downloaded (into Zed's own cache, where Zed looks for it too).

`ZED_EXTENSIONS_DIR` installs into another Zed extensions directory; `WASI_SDK_PATH` uses an
existing wasi-sdk instead of downloading one.

One extension at a time works too: command palette → `zed: install dev extension` → pick its
folder.

### Updating

The extensions run from the clone, so `git pull` updates them. Then press **Rebuild** next to the
extension on the Extensions page (`zed: extensions`) for Zed to load the new version.

### Uninstalling

**Uninstall** next to the extension on the Extensions page. It removes Zed's link to the folder; the
clone stays as it is.

## Repository

Every top-level folder with an `extension.toml` is an extension, with its own README, LICENSE and
version. The installers find extensions by these folders, so a new one needs no change to them.

| Path                                         | Purpose                                                        |
|----------------------------------------------|----------------------------------------------------------------|
| `install.sh`, `install.ps1`                  | The installers.                                                |
| `.github/workflows/install-scripts.yml`      | Runs both installers on macOS, Linux and Windows.              |
| `.github/scripts/test-install.ps1`           | Checks what the installers did; runs locally as well.          |
| `.github/workflows/http-client-extension*.yml` | Tests and releases of the HTTP Client extension.             |
| `.github/workflows/flutter-extension.yml`    | Checks Flutter Tools: snippets, tools, hot reload on each platform. |

## License

[MIT](LICENSE)
