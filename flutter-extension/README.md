# Flutter Tools

Dart and Flutter for [Zed](https://zed.dev), as they work in VS Code:

- **Run and debug on every platform.** With a Dart file open, Zed's run window (`task: spawn`,
  `debugger: start`) offers the app on the iOS Simulator, Android, the web, macOS, Windows and
  Linux: in the terminal, or under the debugger. A simulator or an emulator is started when none
  runs. Nothing is written into the project.
- **Hot reload on save, hot restart on Restart.** Saving a `.dart` file reloads the app, under the
  debugger and in the terminal of a `Flutter: run on ...` task alike; the debugger's Restart (or
  `R` in the terminal) restarts it.
- **Flutter DevTools** for the app under the debugger: the widget inspector with the live widget
  tree, performance, CPU profiler, memory, network and logging, opened from the Run tab in a window
  next to Zed (Zed cannot show web pages itself). Selecting a widget, in the inspector or in the
  app, opens its code in Zed.
- **144 snippets** for widgets, layouts, scrolling, builders, navigation, dialogs, forms,
  animations, logging, Dart declarations and widget tests, with short prefixes (`sfp`, `lvb`,
  `ftb`); each adds the imports it needs (`scf` flutter/material.dart, `log` dart:developer, ...).
- **Code in documentation comments, highlighted.** The code blocks of `///` comments
  (```` ```dart ````, or a fence with no language, which dartdoc takes for Dart) are highlighted as
  Dart; the rest of the comment stays a comment. A theme can set the examples apart from the code
  around them: the Farum Azula themes (`../theme-extension`) darken their colors.
- **The Dart language itself**: everything Zed's [Dart extension](https://github.com/zed-extensions/dart)
  gives (highlighting, the Dart language server, its tasks, the `Dart` debugger), included as it is.

Needs Flutter in `PATH` (or [fvm](https://fvm.app)), nothing else: the extension runs on the Flutter
SDK's own Dart.

## Installation

Zed's Dart extension must go first: two extensions cannot both define Dart. Uninstall it on the
Extensions page, and take `"dart": true` out of `auto_install_extensions` if it is there. Then run
`install.sh` (macOS, Linux) or `install.ps1` (Windows) from the repository root, or command palette
→ `zed: install dev extension` → pick the `flutter-extension` folder. See the
[repository README](../README.md). Settings of the Dart language server stay where they were
(`"lsp": {"dart": ...}`).

## Running the App

Open the run window from a Dart file: `task: spawn` for the **Run** tab, `debugger: start` (F4 in
the default keymap) for **Debug**. Next to the Dart tasks there are, for each platform:

| Tab   | Name                               | What it is                                                     |
|-------|------------------------------------|----------------------------------------------------------------|
| Run   | `Flutter: run on iOS Simulator`    | `flutter run` in the terminal, with r / R / q                  |
| Debug | `Flutter: debug on iOS Simulator`  | The debugger, with hot reload on save and hot restart (below) |

The run button next to a Flutter app's `main()` offers the run tasks too. Each picks its device by
platform:

| Platform      | Device                                                                                 |
|---------------|----------------------------------------------------------------------------------------|
| iOS Simulator | A running simulator; otherwise an iPhone simulator of the newest iOS is booted         |
| Android       | A connected phone or a running emulator; otherwise the first emulator is started      |
| Web           | Chrome or Edge; without either, `web-server`, whose URL opens in any browser           |
| macOS, Windows, Linux | This computer (each only on its own OS)                                        |

They run `lib/main.dart` of the project root, through fvm when the project has a `.fvmrc`. For
anything else (a flavor, another entry point, one particular phone), add a scenario of your own to
`.zed/debug.json`, see [Options](#options).

### Plain Dart

For a project that is not a Flutter app, `task: spawn` offers, next to `dart pub get`,
`dart analyze` and `dart format`:

| Task                       | What it runs                                                          |
|----------------------------|-----------------------------------------------------------------------|
| `dart run: <file>`         | `dart run` for the file open in the editor, in its package; builds first when the project uses `build_runner`; also on the run button next to `main()` |
| `dart run: bin/main.dart`  | `dart run bin/main.dart` in the project root; builds first when the project uses `build_runner` |
| `dart test`                | `dart test` for the whole project (`flutter test` / `fvm flutter test` for a Flutter app) |

The Debug tab offers no `Dart: debug ...` scenario for these tasks yet (that would be a second
debug locator, Rust code in `src/`; see `roadmap/bugs.md` in the repository root). The `Dart`
debug adapter is there, though: a scenario of your own in the project's `.zed/debug.json`
debugs a Dart program:

```json
{
  "label": "Dart: bin/main.dart",
  "adapter": "Dart",
  "type": "dart",
  "request": "launch",
  "program": "bin/main.dart"
}
```

### build_runner

When a project has `build_runner` in the `dev_dependencies` of its `pubspec.yaml` (the workspace
folder itself, and the folders right under it), the Flutter Tools language server starts
`dart run build_runner watch --delete-conflicting-outputs` there once, when the project opens, and
stops it, with what it started, when Zed closes the server. Its output is in Zed's language server
log (`dev: open language server logs`, `flutter-tools`); each `Succeeded` / `Failed` line of a
build is also shown as a message. To turn it off, set in Zed's settings

```json
{ "lsp": { "flutter-tools": { "initialization_options": { "build_runner": false } } } }
```

or start Zed with `FLUTTER_TOOLS_BUILD_RUNNER=off` in the environment. The tasks `dart run
build_runner build` (a single build) and `dart run build_runner watch` (the watch in a terminal
tab, for when the automatic one is off) are there as well.

Once you have run `Flutter: run on <platform>`, Zed no longer offers `Flutter: debug on <platform>`
in the Debug tab until it restarts: Zed leaves the scenarios of tasks already spawned out of the
list. The other platforms' stay, and so does the run task, under its recent ones.

## Hot Reload and Hot Restart

**In a `Flutter: run on ...` task** (the terminal): saving a `.dart` file under the app's `lib/`
hot reloads it (several saves within 200 ms make one reload); `r`, `R` and `q` work as in any
`flutter run`. The runner gives `flutter run` a `--pid-file` and sends it SIGUSR1, Flutter's own
signal for a hot reload, so the terminal stays Flutter's. Generated code counts too: when
`build_runner watch` rewrites a `.dart` file under `lib/`, the app reloads. Not on Windows, which
has no such signals (`r` there); `FLUTTER_TOOLS_HOT_RELOAD_ON_SAVE=off` in Zed's environment
switches it off; a `--pid-file` of your own in the task's arguments is kept and used.

**In a `dart run: <file>` task** (a Dart program on the VM: a server, a CLI): the program runs with
its VM service on, and saving a `.dart` file of its package, or a file under a `lib/resources/`
directory, hot reloads it (`reloadSources`; several saves within 200 ms make one reload), with a
`↻ Reloaded in 55 ms` line in the terminal. Generated code counts: when `build_runner watch`
rewrites `*.spring.dart`, the program reloads again. A **Spring Boot for Dart** application
(`spring_boot_dart` 0.3.2 and later) then restarts itself when its beans, annotations or config
files changed — a new `@GetMapping`, another `server.port` — and keeps running with the new
method bodies otherwise, like Spring Boot DevTools. A compile error is shown only when the code
still does not compile 2 s later (stale generated code is not an error); a reload the VM rejects
for another reason restarts the program. Ctrl+C stops it. `FLUTTER_TOOLS_HOT_RELOAD_ON_SAVE=off`
runs it with plain `dart run`.

**Under the debugger**: start a `Flutter: debug on ...` scenario, or one of your own of the
`Flutter` adapter:

| What you do                              | What happens                                               |
|------------------------------------------|------------------------------------------------------------|
| Save a `.dart` file under `lib/`         | Hot reload: the new code runs, the app keeps its state     |
| **Restart** in the debug panel           | Hot restart: the app starts over, without a rebuild        |
| **Stop**                                 | The app quits                                              |

Breakpoints, stepping, variables and the debug console work as usual: they are Flutter's own
debugger's. With autosave on (`"autosave": {"after_delay": {"milliseconds": 1000}}`), the app
reloads a second after you stop typing.

Extensions cannot add key bindings, so for a hot restart from the keyboard add one to your keymap
(command palette → `zed: open keymap file`). This one saves the file, then restarts the app:
<kbd>Option</kbd>+<kbd>Cmd</kbd>+<kbd>S</kbd> on macOS, <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>S</kbd>
on Linux and Windows.

```json
[
  {
    "context": "Editor",
    "bindings": {
      "cmd-alt-s": ["action::Sequence", ["workspace::Save", "debugger::Restart"]],
      "ctrl-alt-s": ["action::Sequence", ["workspace::Save", "debugger::Restart"]]
    }
  }
]
```

Keep the line for your OS, or both if one keymap serves several machines.

### Options

A scenario of your own for the `Flutter` adapter, in the project's `.zed/debug.json`:

```json
{
  "label": "Flutter: Android (dev)",
  "adapter": "Flutter",
  "request": "launch",
  "program": "lib/main_dev.dart",
  "target": "android",
  "toolArgs": ["--flavor", "dev"]
}
```

| Field             | Default          | Meaning                                                           |
|-------------------|------------------|-------------------------------------------------------------------|
| `target`          | none             | `ios-simulator`, `android`, `web`, `macos`, `windows`, `linux`: a device of that kind, as above |
| `deviceId`        | Flutter's choice | One device by id (`flutter devices`); wins over `target`          |
| `program`         | `lib/main.dart`  | The entry point, relative to `cwd`                                |
| `cwd`             | the project root | The Flutter project's directory (in a monorepo, the app's folder) |
| `flutterMode`     | `debug`          | `debug`, `profile` or `release`; reload and restart need `debug`  |
| `args`            | `[]`             | Arguments for the app                                             |
| `toolArgs`        | `[]`             | Arguments for `flutter run`: `--flavor dev`, `--dart-define=K=V`  |
| `env`             | `{}`             | Environment variables for Flutter                                 |
| `hotReloadOnSave` | `true`           | `false` reloads only on Restart                                   |
| `devTools`        | `true`           | `false`: no DevTools for this app (see below)                     |
| `useFvm`          | `true` with a `.fvmrc`, else `false` | Run Flutter through `fvm`               |
| `request`         | `launch`         | `attach` (with `vmServiceUri`) attaches to a running app          |

## Flutter DevTools

DevTools works with an app running under the debugger:

1. Start the app in the **Debug** tab (`debugger: start`): `Flutter: debug on <platform>`.
2. Open DevTools from the **Run** tab (`task: spawn`), with one of these tasks:

| Task                                 | Shows                                                           |
|--------------------------------------|-----------------------------------------------------------------|
| `Flutter DevTools: Widget Inspector` | The widget tree, live, with each widget's properties and layout |
| `Flutter DevTools: Performance`      | Frame times, jank, rebuilds and the timeline                    |
| `Flutter DevTools: CPU Profiler`     | Where the time goes, by function                                |
| `Flutter DevTools: Memory`           | The heap, allocations and leaks                                 |
| `Flutter DevTools: Network`          | HTTP requests and their responses                               |
| `Flutter DevTools: Logging`          | Logs, `log()`'s included, and framework events                  |

The tasks are in the Run tab because the Debug tab only starts debug sessions. Without an app
under the debugger, a task says so in the terminal and opens nothing; an app started from the Run
tab (`Flutter: run on ...`) has no DevTools here, but `flutter run` prints a DevTools link in its
terminal.

The page opens in a window of its own, next to Zed: Chrome, Edge or Chromium in app mode (no tabs,
no address bar), or the default browser without them. Every page is a tab of that window, so one
task is enough to reach all of them. The debug console prints DevTools' address too.

**Selecting a widget opens its code in Zed**: a click in the inspector's tree, or, with *Select
Widget Mode* on (the inspector's toolbar), a tap on the widget in the app. This needs Zed's command
line tool, `zed`: on macOS, command palette → `cli: install`; elsewhere it must be on `PATH`
(Zed's Linux installer puts it in `~/.local/bin`).

DevTools is the Dart SDK's own (`dart devtools`), served for each app the debugger runs, and
stopped with it; `"devTools": false` in a scenario of your own turns it off. A task finds the app
of its project through `~/.zed-flutter/sessions/`, where the debugger records each app's DevTools
address.

### Why DevTools Is Not Inside Zed

DevTools is a web app. VS Code shows it in a panel because VS Code can show web pages (webviews);
Zed cannot, and a Zed extension cannot draw panels or windows of its own: it can only give Zed data
(completions, highlighting, tasks, debuggers). So, until Zed can show web pages, DevTools lives in
a window next to it. A browser that draws in the terminal (such as Carbonyl) could show it in Zed's
terminal, but at a resolution too coarse for DevTools to be usable.

## How It Works
## How It Works

- **The Dart language** ([`languages/dart`](languages/dart), [`src/dart.rs`](src/dart.rs)) is the
  Dart extension's, with these changes: the `Flutter: run on <platform>` tasks in `tasks.json`,
  `debuggers` in `config.toml`, a newer [tree-sitter-dart](https://github.com/UserNobody14/tree-sitter-dart)
  that parses Dart 3.12/3.13 (primary constructors such as `class A({required this.x})`), and in
  `highlights.scm` a factory constructor's name highlighted as a method's (`factory fromJson()`).
  Only the extension that defines a language can add tasks to it, which is why this one defines
  Dart.
- **Code in documentation comments** ([`languages/dartdoc`](languages/dartdoc),
  [`tree-sitter-dartdoc`](tree-sitter-dartdoc)). The Dart grammar gives a `///` comment no node
  for its text, only the whole line with its `///`, and Zed injects whole nodes. So the Dart
  language injects all of a file's `///` comments at once into `Dartdoc`, a small grammar of this
  extension: Zed hands them over a line each, and it finds the fenced code blocks and gives each
  line of their code a node of its own, without the `///`. Those lines, a block at a time, are
  injected into the language the fence names, Dart for none: `Dartdoc Dart`, `Dartdoc Java`,
  `Dartdoc Rust`, `Dartdoc JavaScript`, `Dartdoc TypeScript`, `Dartdoc Python`
  (`languages/dartdoc-<name>`). Each is that language's grammar and `highlights.scm`, and nothing
  else, so that an example's classes stay out of the outline and its `main()` gets no run button.
  Their captures have `.dartdoc` after them (`keyword.dartdoc`), which a theme styles as `keyword`
  unless it has a style for them; a block's lines and fences are `dartdoc.code`. All are hidden: in
  a comment, the language is still Dart (comment toggling, `///` on a new line). Java's grammar is
  the Java extension's, so Java examples need it installed; the others come with Zed. Blocks of
  other languages (`kotlin`, `yaml`) stay comments.
- **The `flutter` debug locator** ([`src/lib.rs`](src/lib.rs)): Zed asks it about each task, and
  it turns each `Flutter: run on <platform>` into `Flutter: debug on <platform>`, a scenario of the
  `Flutter` adapter with that `target`.

The rest are small Dart programs in [`tool/`](tool), run by the Dart of your Flutter SDK; the
extension writes them into its work directory as they start. Nothing is downloaded or built.

- **[`flutter_proxy.dart`](tool/flutter_proxy.dart), the `Flutter` debug adapter.** Flutter's debug
  adapter, `flutter debug_adapter`, can hot reload, but only when the editor asks it to: VS Code
  does on save, Zed does not. This proxy stands between Zed and that adapter and passes every
  message through unchanged; once the app runs, it sends Flutter that `hotReload` request itself
  when a `.dart` file under `lib/` changes on disk (several changes within 200 ms make one reload).
  For a `target`, it finds or starts the device before Flutter gets the launch. The debugger's
  Restart reaches Flutter as a hot restart by itself. When Flutter reports the app's VM service, it
  starts DevTools and records it in `~/.zed-flutter/sessions/`; when Flutter reports a selected
  widget (a `navigate` tool event), it opens the widget's file in Zed at its line. And since
  Flutter's adapter answers a `disconnect` by killing `flutter run`, which leaves a desktop app
  running, the proxy sends it a `terminate` first, so that Stop quits the app.
- **[`flutter_run.dart`](tool/flutter_run.dart), the runner of the run tasks.** It finds or starts
  the device, then runs `flutter run -d <device>` in the terminal.
- **[`flutter_devtools.dart`](tool/flutter_devtools.dart), the program of the DevTools tasks.** It
  finds the app of the project in `~/.zed-flutter/sessions/` and opens the page for it.
- **[`flutter_lsp.dart`](tool/flutter_lsp.dart), a language server next to the Dart one.** It
  offers the snippets as completions carrying edits that add their imports: Zed's own snippets
  can only insert at the cursor. And since a task cannot name the extension's directory,
  which differs between OSes, the run and DevTools tasks start `~/.zed-flutter/flutter_run.dart`
  and `~/.zed-flutter/flutter_devtools.dart`, which this server keeps there and up to date.
  It offers a snippet only for the word being typed (`sc` offers `sc`, `scf`, ...), never after
  `{`, `(` or `.`, and runs `build_runner watch` for the projects that use it (above).
- [`devices.dart`](tool/devices.dart) finds and starts devices; [`devtools.dart`](tool/devtools.dart)
  starts DevTools, keeps the sessions, opens windows and Zed;
  [`protocol.dart`](tool/protocol.dart) frames the messages of the proxy and the server.

## Snippets

Type a prefix and pick the snippet from the completion menu; the menu shows only the snippets
that start with what is typed, and nothing after `{`, `(` or `.`. **ctrl-space** (Zed's
`editor::ShowCompletions`, the same on every platform) lists them all; for option-space on a Mac
add to `keymap.json`:

```json
[{ "context": "Editor", "bindings": { "alt-space": "editor::ShowCompletions" } }]
```

Prefixes are short, mostly the first letters of the words: `sfp` is a **s**tate**f**ul widget with a **p**arameter, `lvb` a
**L**ist**V**iew.**b**uilder, `ftb` a **F**u**t**ure**B**uilder. **Tab** moves to the next field; a
field that appears more than once (a class name, say) is edited everywhere at once; fields with a
fixed set of values (`MainAxisAlignment.start`, `BoxFit.cover`, ...) offer them as a list. Every
field has a default, and with the defaults the code compiles once the names it refers to
(`items`, `_controller`, ...) exist.

Picking a snippet also adds the imports it needs when the file has none that brings them: `scf`
adds `package:flutter/material.dart`, `sfp` `package:flutter/widgets.dart` (unless material.dart or
cupertino.dart is there, which bring it), `log` `dart:developer` (with the other `dart:` imports),
`tw` `flutter_test` and material.dart. That is why the snippets come from the extension's language
server rather than from a Zed snippet file: Zed's snippets can only insert at the cursor.

The Dart language server has snippets of its own, so these do not repeat them: `stless`, `stful`,
`stanim`, `class`, `main`, `fun`, `for`, `forin`, `if`, `ife`, `switch`, `try`, `while`, `do`,
`test`, `group`.

### Logging and async

| Prefix  | Inserts                                                         | Import                                   |
|---------|-----------------------------------------------------------------|------------------------------------------|
| `log`   | `log()`, shown in the debug console and DevTools                | `dart:developer`                         |
| `loge`  | `log()` of an error with its stack trace                        | `dart:developer`                         |
| `logd`  | `debugPrint()`, printed in debug builds only                    | `flutter/foundation.dart` (or any Flutter library) |
| `pr`    | `print()`                                                       |                                          |
| `tm`    | Start a Timer                                                   | `dart:async`                             |
| `tmp`   | Start a periodic Timer                                          | `dart:async`                             |
| `cpl`   | Create a Completer                                              | `dart:async`                             |
| `sctrl` | A broadcast StreamController field                              | `dart:async`                             |

### App

| Prefix | Inserts |
|---|---|
| `mapp` | A MaterialApp with a home screen |
| `capp` | A CupertinoApp with a home screen |
| `mna` | main() that initializes bindings before runApp |
| `thd` | A Material 3 ThemeData from a seed color |

### Widget classes

| Prefix | Inserts |
|---|---|
| `slc` | A StatelessWidget that wraps a child |
| `sfp` | A StatefulWidget with a field read through widget. |
| `inh` | An InheritedWidget with of() and maybeOf() |
| `cn` | A ChangeNotifier with one notifying field |
| `cpt` | A CustomPainter |
| `ccl` | A CustomClipper&lt;Path&gt; |

### State members

| Prefix | Inserts |
|---|---|
| `ins` | Override initState |
| `ds` | Override dispose |
| `dcd` | Override didChangeDependencies |
| `duw` | Override didUpdateWidget |
| `dea` | Override deactivate |
| `bld` | Override build |
| `ac` | An AnimationController (the State needs a TickerProvider mixin; dispose it) |
| `tc` | A TextEditingController (dispose it) |
| `sc` | A ScrollController (dispose it) |
| `fn` | A FocusNode (dispose it) |
| `fk` | A GlobalKey&lt;FormState&gt; |
| `gt` | A getter |
| `st` | A setter |

### Statements

| Prefix | Inserts |
|---|---|
| `ss` | Call setState |
| `ifm` | Stop when the State is no longer mounted (after an await) |
| `ifcm` | Stop when the BuildContext is no longer mounted (after an await) |
| `np` | Push a MaterialPageRoute |
| `npn` | Push a named route |
| `npr` | Replace the current route |
| `npa` | Push a route and remove all the others |
| `npo` | Pop the current route |
| `npu` | Pop back to the first route |
| `sd` | Show an AlertDialog with cancel and confirm actions |
| `sbs` | Show a modal bottom sheet |
| `ssb` | Show a SnackBar through the ScaffoldMessenger |
| `pfc` | Run code after the current frame |
| `fd` | Wait for a duration |
| `tcf` | try with catch and finally |
| `uf` | Dismiss the keyboard |
| `ifc` | Match a pattern |
| `fid` | Loop over a list with its indexes |

### Layout

| Prefix | Inserts |
|---|---|
| `scf` | A Scaffold with an AppBar |
| `ab` | An AppBar with actions |
| `col` | A Column |
| `row` | A Row |
| `stk` | A Stack |
| `pos` | A Positioned child of a Stack |
| `cen` | A Center |
| `pd` | Padding on all sides |
| `pds` | Horizontal and vertical padding |
| `sbh` | Vertical space |
| `sbw` | Horizontal space |
| `sbx` | A SizedBox with a child |
| `ct` | A Container |
| `dct` | A Container with a rounded BoxDecoration |
| `ex` | An Expanded |
| `fx` | A Flexible |
| `al` | An Align |
| `wr` | A Wrap |
| `sa` | A SafeArea |
| `crr` | Clip with rounded corners |
| `cd` | A Card with padding |
| `lt` | A ListTile |
| `dv` | A Divider |
| `cpi` | A centered progress indicator |
| `vis` | Show a child conditionally |

### Scrolling

| Prefix | Inserts |
|---|---|
| `lv` | A ListView of children |
| `lvb` | A lazily built ListView |
| `lvs` | A ListView with separators |
| `gvb` | A lazily built grid with a fixed column count |
| `gvc` | A grid of children with a fixed column count |
| `scv` | A scrollable Column |
| `csv` | A CustomScrollView with an app bar and a list |
| `slb` | A lazily built sliver list |
| `sla` | A box widget inside a CustomScrollView |
| `slp` | Padding around a sliver |
| `ri` | Pull to refresh |
| `pvb` | A lazily built PageView |
| `dtc` | A DefaultTabController with a TabBar and a TabBarView |

### Builders

| Prefix | Inserts |
|---|---|
| `bd` | A Builder for a new BuildContext |
| `lb` | Build from the parent constraints |
| `ftb` | Build from a Future, with loading and error states |
| `stb` | Build from a Stream, with loading and error states |
| `vlb` | Rebuild when a ValueListenable changes |
| `lnb` | Rebuild when a Listenable notifies |
| `anb` | Rebuild on every animation tick |

### Content

| Prefix | Inserts |
|---|---|
| `tx` | A Text with a theme style |
| `rt` | Text with several styles |
| `ic` | An Icon |
| `img` | An image from the assets |
| `imgn` | An image from the network, with loading and error states |
| `hr` | A Hero animation |

### Input

| Prefix | Inserts |
|---|---|
| `gd` | Detect taps |
| `iw` | A tappable area with a ripple |
| `eb` | An ElevatedButton |
| `flb` | A FilledButton |
| `ob` | An OutlinedButton |
| `tb` | A TextButton |
| `ib` | An IconButton |
| `fab` | A FloatingActionButton |
| `tf` | A TextField with a controller |
| `tff` | A TextFormField with a validator |
| `frm` | A Form with a submit button |
| `cbt` | A checkbox with a label |
| `swt` | A switch with a label |
| `sl` | A Slider |
| `dd` | A DropdownButton |

### Animation

| Prefix | Inserts |
|---|---|
| `anc` | A Container that animates its changes |
| `ano` | Fade a child in and out |
| `ans` | Animate between children |

### Theme and screen

| Prefix | Inserts |
|---|---|
| `mqs` | The size of the screen |
| `th` | The current theme |
| `tt` | The text theme |
| `cs` | The color scheme |

### Dart

| Prefix | Inserts |
|---|---|
| `imm` | Import package:flutter/material.dart |
| `imc` | Import package:flutter/cupertino.dart |
| `imw` | Import package:flutter/widgets.dart |
| `ims` | Import package:flutter/services.dart |
| `ima` | Import dart:async |
| `mdl` | An immutable class with copyWith, JSON, equality and toString |
| `en` | An enum with a field and a constructor |
| `sld` | A sealed class with two subclasses |
| `ext` | An extension |
| `mx` | A mixin |
| `sgl` | A class with a single instance |
| `td` | A type alias |
| `fa` | A function returning a Future |
| `fs` | A function generating a Stream |
| `todo` | A TODO comment |

### Tests

| Prefix | Inserts |
|---|---|
| `tw` | A widget test |
| `su` | Run before each test |
| `tdn` | Run after each test |
| `sua` | Run once before all tests |
| `tp` | Tap a widget and settle |
| `et` | Type into a text field |
| `ef` | Expect a widget to be found |

## Development

```sh
python3 tests/check_snippets.py     # the snippets; needs flutter in PATH
python3 tests/check_proxy.py        # the debug adapter proxy, against a stand-in for Flutter
python3 tests/check_devtools.py     # DevTools: served, its pages opened, widgets opened in Zed
python3 tests/check_queries.py      # the queries against the grammars; needs cargo and npx
python3 tests/check_tasks.py        # the run tasks, the debug locator and the targets agree
python3 tests/check_lsp.py          # the language server: snippets and their imports, build_runner, the runner
python3 tests/check_run.py          # the runner of the run tasks, against a stand-in for Flutter
python3 tests/check_hot_reload.py [--device <target>]   # a real app: reload, restart, DevTools, Stop
```

`check_queries.py` builds the grammar `extension.toml` pins, compiles every query of
`languages/dart` against it (Zed drops a query that does not compile, and with `highlights.scm` all
highlighting), and checks the captures marked in `tests/queries/highlights.dart`: a `^` under a
token names the capture expected there. Add a line there with each change to `highlights.scm`.
It also checks the code blocks of documentation comments: that `tree-sitter-dartdoc/src` is what its
`grammar.js` generates and its corpus (`test/corpus`) passes, and that in
`tests/queries/doc_comments.dart`, with its layers built the way Zed builds them, the marks of code
blocks get their language's captures and the rest `comment.documentation`. After a change to
`grammar.js`, regenerate with `npx tree-sitter-cli@0.27.1 generate` in `tree-sitter-dartdoc`; after
one to `languages/dart/highlights.scm`, or to the other languages' in `tests/doc_languages` (Zed's
and the Java extension's, copied with where from), run `python3 tests/check_queries.py --update`: it
writes `languages/dartdoc-<name>` (each capture suffixed `.dartdoc`), `languages/dartdoc/injections.scm`
and the `.dartdoc` styles of the Farum Azula themes: the colors of the language's, darkened. A
language of examples is a line of `DOC_LANGUAGES` there, its grammar a line of
`tests/queries/Cargo.toml` and of `grammar()` in `tests/queries/src/main.rs`.

`check_hot_reload.py` creates a Flutter app, runs it through the proxy and Flutter's adapter the way
Zed does (on this OS's desktop, or `--device ios-simulator`, `android`, ...), saves a change and
checks that the new code runs without a restart, then sends Restart and checks that `main` runs
again, that DevTools is served and a widget selected in the inspector opens its code in Zed, and
that Stop quits the app. The other checks need no device: `tests/fake_flutter.py`
stands in for `flutter`, its debug adapter, its device list and its emulators, and
`tests/fake_devtools.py` for DevTools, a browser and `zed`. CI runs them on Linux, macOS and Windows, and the real
app on macOS, the iOS Simulator, Windows, Linux and an Android emulator.

### Snippets

Snippets live in `tool/snippets.json`, in Zed's snippet syntax: `$1`, `${1:default}`,
`${1|one,two|}`, `$0` for the final cursor position. A `$` that does not start a field must be
written `\\$` in the JSON (`'\\$name'` for Dart's `'$name'`): Zed rejects the whole snippet
otherwise. Inside a field's default, a `}` is written `\\}`.

`tests/check_snippets.py` checks every snippet:

1. its prefix is its own, and not one of the Dart language server's;
2. Zed can parse it;
3. each repeated field has the same default everywhere;
4. expanded with its defaults and put where it is typed, it is Dart without errors (`flutter
   analyze` on a scratch Flutter project) with its `imports`, and needs no fewer.

Each snippet has a `prefix`, a `description`, a `context`, where it is typed (`top`, `member` of a
class or a `State`, `statement`, `widget`, `expr`, `test`, `tester` inside a `testWidgets`), the
`imports` it needs, and the `body`. Leave `imports` out of a new snippet and run
`python3 tests/check_snippets.py --update`: it tries no import, then one Dart or Flutter library,
then a Flutter library with a Dart one, and writes the first that compiles.

## License

[MIT](LICENSE)
