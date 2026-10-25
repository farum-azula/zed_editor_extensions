// Runs the Flutter app of the current directory on a run target, in the terminal: `flutter run`
// with its keys (r hot reload, R hot restart, q quit), on a device it picks or starts first. In a
// monorepo or a pub workspace, the app is the one under the current directory (see flutterApp in
// devices.dart), and `flutter run` runs in its folder.
//
// Saving a `.dart` file under the app's `lib/` hot reloads it, as under the debugger: `flutter
// run` gets a `--pid-file`, and a change (several within 200 ms make one) sends its process
// SIGUSR1, Flutter's own signal for a hot reload. The terminal stays Flutter's (colors, keys).
// Not on Windows, which has no such signals; FLUTTER_TOOLS_HOT_RELOAD_ON_SAVE=off switches it off.
//
//     dart flutter_run.dart <target> [flutter run arguments...]
//
// <target> is one of ios-simulator, android, web, macos, windows, linux (see devices.dart).

import 'dart:async';
import 'dart:io';

import 'devices.dart';

/// Changes closer together than this make one hot reload.
const debounce = Duration(milliseconds: 200);

Future<void> main(List<String> args) async {
  if (args.isEmpty) {
    stderr.writeln(
      'usage: dart flutter_run.dart <${targets.keys.join('|')}> [flutter run args...]',
    );
    exit(64);
  }
  final Directory app;
  final List<String> flutter;
  final String device;
  try {
    app = flutterApp(Directory.current);
    flutter = flutterCommand(app);
    device = await resolveDevice(args.first, flutter, stdout.writeln);
  } on DeviceException catch (error) {
    stderr.writeln(error);
    exit(1);
  }
  final runArgs = args.skip(1).toList();
  final reloader = reloadOnSave ? HotReloadOnSave.start(app, runArgs) : null;
  final run = await Process.start(
    flutter.first,
    [...flutter.skip(1), 'run', '-d', device, ...runArgs, ...?reloader?.flutterArgs],
    workingDirectory: app.path,
    mode: ProcessStartMode.inheritStdio,
    runInShell: Platform.isWindows,
  );
  final code = await run.exitCode;
  await reloader?.stop();
  exit(code);
}

/// Whether saving reloads: not on Windows (no signals), not when switched off.
bool get reloadOnSave =>
    !Platform.isWindows &&
    (Platform.environment['FLUTTER_TOOLS_HOT_RELOAD_ON_SAVE'] ?? '').toLowerCase() != 'off';

/// Watches the app's `lib/` and sends `flutter run` SIGUSR1 (a hot reload) when a `.dart` file
/// changes.
class HotReloadOnSave {
  HotReloadOnSave._(this._pidFile, this.flutterArgs, this._ownPidFile);

  /// Starts watching [app]; [runArgs] are the user's `flutter run` arguments (their own
  /// `--pid-file` is used when they give one).
  factory HotReloadOnSave.start(Directory app, List<String> runArgs) {
    final given = _pidFileOf(runArgs);
    final pidFile = given != null
        ? File(given.startsWith('/') ? given : '${app.path}/$given')
        : File('${Directory.systemTemp.createTempSync('flutter_run').path}/flutter.pid');
    final reloader = HotReloadOnSave._(
      pidFile,
      given == null ? ['--pid-file', pidFile.path] : const [],
      given == null,
    );
    final lib = Directory('${app.path}/lib');
    reloader._watch = (lib.existsSync() ? lib : app).watch(recursive: true).listen(
      (event) {
        if (event.path.endsWith('.dart')) {
          reloader._pending?.cancel();
          reloader._pending = Timer(debounce, reloader._reload);
        }
      },
      onError: (Object error) =>
          stderr.writeln('Not watching ${lib.path} for hot reload on save: $error'),
    );
    return reloader;
  }

  final File _pidFile;
  final bool _ownPidFile;

  /// What `flutter run` gets on top of the user's arguments.
  final List<String> flutterArgs;

  StreamSubscription<FileSystemEvent>? _watch;
  Timer? _pending;

  void _reload() {
    // Before Flutter wrote the file (still building), there is nothing to reload yet.
    if (!_pidFile.existsSync()) {
      return;
    }
    final pid = int.tryParse(_pidFile.readAsStringSync().trim());
    if (pid != null) {
      Process.killPid(pid, ProcessSignal.sigusr1);
    }
  }

  /// Stops watching and removes its own pid file.
  Future<void> stop() async {
    _pending?.cancel();
    await _watch?.cancel();
    if (_ownPidFile) {
      try {
        _pidFile.parent.deleteSync(recursive: true);
      } on FileSystemException {
        // Already gone.
      }
    }
  }

  static String? _pidFileOf(List<String> args) {
    for (var i = 0; i < args.length; i++) {
      if (args[i] == '--pid-file' && i + 1 < args.length) {
        return args[i + 1];
      }
      if (args[i].startsWith('--pid-file=')) {
        return args[i].substring('--pid-file='.length);
      }
    }
    return null;
  }
}
