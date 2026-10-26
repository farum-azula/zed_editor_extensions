// Runs a Dart file the way an IDE does: building first when the project generates code.
//
//     dart dart_run.dart <file> [program arguments...]
//
// In the package of the file (the nearest pubspec.yaml above it), a pubspec with `build_runner`
// in `dev_dependencies` gets `dart run build_runner build --delete-conflicting-outputs` first,
// since generated code that nobody watches is stale and the program would not compile. A failed
// build stops here with its exit code. Then `dart run <file> <arguments>` runs in the package
// directory, and its exit code is this program's.
//
// A Flutter package (`flutter: sdk: flutter` in `dependencies`) cannot run on the Dart VM, so the
// file runs with `flutter run -t <file> <arguments>` instead (`fvm flutter` under FVM): a main()
// that does not import package:flutter itself, say one that gets Flutter through another package,
// still runs as the app it is.
//
// A Dart program runs with the VM service on, and is hot reloaded on save: a change to a `.dart`
// file of the package (generated ones included, so `build_runner watch` rewriting
// `*.spring.dart` counts) or to a file under a `lib/resources/` directory (several within 200 ms
// make one) calls `reloadSources` on its isolates. Then, if the program registered the service
// extension `ext.spring_boot.reload` (a Spring Boot for Dart application does), it is called: the
// application restarts when its generated code changed (beans, annotations, config files), like
// Spring Boot DevTools. A reload the VM rejects for another reason than a compile error restarts
// the program. FLUTTER_TOOLS_HOT_RELOAD_ON_SAVE=off switches it off (the program then runs with
// plain `dart run`).

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'devices.dart' show flutterCommand;

Future<void> main(List<String> args) async {
  if (args.isEmpty) {
    stderr.writeln('usage: dart dart_run.dart <file> [program args...]');
    exit(64);
  }
  final file = File(args.first).absolute;
  final package = packageDirectory(file.parent);
  if (package != null && usesBuildRunner(package)) {
    final build = await dart([
      'run',
      'build_runner',
      'build',
      '--delete-conflicting-outputs',
    ], package);
    if (build != 0) {
      exit(build);
    }
  }
  final directory = package ?? file.parent;
  if (package != null && isFlutterPackage(package)) {
    final flutter = flutterCommand(package);
    exit(
      await command(flutter.first, [
        ...flutter.skip(1),
        'run',
        '-t',
        file.path,
        ...args.skip(1),
      ], directory),
    );
  }
  if (hotReloadOnSave(Platform.environment)) {
    exit(await runWithHotReload(file, args.skip(1).toList(), directory));
  }
  exit(await dart(['run', file.path, ...args.skip(1)], directory));
}

/// Whether to hot reload on save: unless FLUTTER_TOOLS_HOT_RELOAD_ON_SAVE is off (or false, 0, no).
bool hotReloadOnSave(Map<String, String> environment) {
  final value = environment['FLUTTER_TOOLS_HOT_RELOAD_ON_SAVE']?.trim().toLowerCase();
  return !const {'off', 'false', '0', 'no'}.contains(value);
}

/// Runs [file] on the Dart VM with its VM service on, hot reloading it on save (see the top of
/// this file); its exit code. A rejected reload starts it again.
Future<int> runWithHotReload(File file, List<String> args, Directory directory) async {
  final temp = Directory.systemTemp.createTempSync('dart_run_');
  try {
    while (true) {
      final info = File('${temp.path}${Platform.pathSeparator}vm_service.json');
      if (info.existsSync()) {
        info.deleteSync();
      }
      final process = await Process.start(
        'dart',
        [
          '--enable-vm-service=0',
          '--no-dds',
          '--write-service-info=${info.uri}',
          file.path,
          ...args,
        ],
        workingDirectory: directory.path,
        mode: ProcessStartMode.inheritStdio,
        runInShell: Platform.isWindows,
      );
      // Stopping the task stops the program too.
      final signals = [
        for (final signal in [ProcessSignal.sigint, if (!Platform.isWindows) ProcessSignal.sigterm])
          signal.watch().listen((_) => process.kill(ProcessSignal.sigterm)),
      ];
      var restart = false;
      final reloader = HotReloader(
        info,
        directory,
        onRejected: () {
          restart = true;
          process.kill();
        },
      )..start();
      final code = await process.exitCode;
      await reloader.stop();
      for (final subscription in signals) {
        await subscription.cancel();
      }
      if (!restart) {
        return code;
      }
      stderr.writeln('${_dim}Restarting ${file.path}$_reset');
    }
  } finally {
    temp.deleteSync(recursive: true);
  }
}

const _dim = '\x1B[2m';
const _reset = '\x1B[0m';
const _red = '\x1B[31m';

/// Watches [package] and hot reloads the program whose VM service [info] names.
final class HotReloader {
  HotReloader(this.info, this.package, {required this.onRejected});

  /// The file the VM writes its service address into (`--write-service-info`).
  final File info;

  /// The package directory of the program.
  final Directory package;

  /// Called when the VM rejects a reload for another reason than a compile error.
  final void Function() onRejected;

  StreamSubscription<FileSystemEvent>? _watch;
  Timer? _timer;

  // A compile error right after a save is often only stale generated code that
  // `build_runner watch` is rewriting: it is shown when no reload succeeded since.
  Timer? _pendingError;
  VmService? _service;
  var _reloading = false;
  var _again = false;
  var _stopped = false;

  /// Starts watching.
  void start() {
    try {
      _watch = package.watch(recursive: true).listen((event) {
        if (watched(event.path, package.path)) {
          _timer?.cancel();
          _timer = Timer(const Duration(milliseconds: 200), _reload);
        }
      }, onError: (Object error) => stderr.writeln('Not watching for hot reload on save: $error'));
    } on FileSystemException catch (error) {
      stderr.writeln('Not watching for hot reload on save: ${error.message}');
    }
  }

  /// Stops watching and closes the VM service connection.
  Future<void> stop() async {
    _stopped = true;
    _timer?.cancel();
    _pendingError?.cancel();
    await _watch?.cancel();
    await _service?.close();
  }

  Future<void> _reload() async {
    if (_stopped) {
      return;
    }
    if (_reloading) {
      _again = true;
      return;
    }
    _reloading = true;
    try {
      await _reloadOnce();
    } on Object catch (error) {
      if (!_stopped) {
        stderr.writeln('${_red}Hot reload failed: $error$_reset');
      }
      await _service?.close();
      _service = null;
    } finally {
      _reloading = false;
    }
    if (_again && !_stopped) {
      _again = false;
      await _reload();
    }
  }

  Future<void> _reloadOnce() async {
    final service = _service ??= await VmService.connect(await _serviceUri());
    final watch = Stopwatch()..start();
    final vm = await service.call('getVM');
    var restarted = 0;
    for (final isolate in (vm['isolates'] as List? ?? const []).cast<Map<String, Object?>>()) {
      final id = isolate['id']! as String;
      final report = await service.call('reloadSources', {'isolateId': id});
      if (report['success'] != true) {
        final notices = (report['notices'] as List? ?? const [])
            .cast<Map<String, Object?>>()
            .map((n) => '${n['message'] ?? ''}'.trim())
            .where((m) => m.isNotEmpty)
            .toList();
        final reason = notices.isEmpty ? 'rejected by the VM' : notices.join('\n');
        if (isCompileError(reason)) {
          _pendingError?.cancel();
          _pendingError = Timer(const Duration(seconds: 2), () {
            stderr.writeln('${_red}Hot reload: the code does not compile$_reset\n$reason');
          });
        } else {
          stderr.writeln('${_red}Hot reload rejected: $reason$_reset');
          onRejected();
        }
        return;
      }
      final details = await service.call('getIsolate', {'isolateId': id});
      final extensions = (details['extensionRPCs'] as List? ?? const []).cast<String>();
      if (extensions.contains(springReloadExtension)) {
        final result = await service.call(springReloadExtension, {'isolateId': id});
        restarted += (result['restarted'] as num?)?.toInt() ?? 0;
      }
    }
    _pendingError?.cancel();
    stderr.writeln(
      '$_dim↻ Reloaded in ${watch.elapsedMilliseconds} ms'
      '${restarted > 0 ? '; the application restarted (its beans or config changed)' : ''}$_reset',
    );
  }

  Future<Uri> _serviceUri() async {
    for (var i = 0; i < 200; i++) {
      if (info.existsSync()) {
        final text = info.readAsStringSync();
        if (text.isNotEmpty) {
          final uri = (jsonDecode(text) as Map<String, Object?>)['uri'];
          if (uri is String) {
            return Uri.parse(uri);
          }
        }
      }
      await Future<void>.delayed(const Duration(milliseconds: 50));
    }
    throw StateError('the program did not open its VM service');
  }
}

/// The service extension of a Spring Boot for Dart application that restarts it after a reload
/// when its generated code changed.
const springReloadExtension = 'ext.spring_boot.reload';

/// Whether a change of [path] (in the package at [package]) hot reloads: a `.dart` file, or a file
/// under a `lib/resources/` directory (config files the generator embeds); not in `.dart_tool/`,
/// `build/` or hidden directories.
bool watched(String path, String package) {
  final relative = path.startsWith(package) ? path.substring(package.length) : path;
  final parts = relative.split(RegExp(r'[\\/]')).where((p) => p.isNotEmpty).toList();
  if (parts.isEmpty || parts.any((p) => p.startsWith('.') || p == 'build')) {
    return false;
  }
  if (parts.last.endsWith('.dart')) {
    return true;
  }
  for (var i = 0; i + 1 < parts.length; i++) {
    if (parts[i] == 'lib' && parts[i + 1] == 'resources' && i + 2 < parts.length) {
      return true;
    }
  }
  return false;
}

/// Whether a rejected reload's [reason] is a compile error (fixed by saving again).
bool isCompileError(String reason) {
  final text = reason.toLowerCase();
  return text.contains('error:') || text.contains('compil') || text.contains('expected');
}

/// A connection to a Dart VM service (JSON-RPC over a WebSocket), enough for hot reload.
final class VmService {
  VmService._(this._socket) {
    _socket.listen(
      (message) {
        final json = jsonDecode(message as String) as Map<String, Object?>;
        final pending = _pending.remove(json['id']);
        if (pending == null) {
          return;
        }
        final error = json['error'];
        if (error is Map) {
          pending.completeError(
            StateError(
              '${error['message']}${error['data'] is Map ? ': ${(error['data'] as Map)['details'] ?? ''}' : ''}',
            ),
          );
        } else {
          pending.complete((json['result'] as Map?)?.cast<String, Object?>() ?? const {});
        }
      },
      onDone: () {
        for (final pending in _pending.values) {
          pending.completeError(StateError('the VM service closed'));
        }
        _pending.clear();
      },
    );
  }

  /// Connects to the service at [uri] (its `http://` address).
  static Future<VmService> connect(Uri uri) async {
    final path = uri.path.endsWith('/') ? uri.path : '${uri.path}/';
    final socket = await WebSocket.connect(uri.replace(scheme: 'ws', path: '${path}ws').toString());
    return VmService._(socket);
  }

  final WebSocket _socket;
  final _pending = <Object?, Completer<Map<String, Object?>>>{};
  var _next = 0;

  /// Calls [method] with [params]; its result.
  Future<Map<String, Object?>> call(String method, [Map<String, Object?> params = const {}]) {
    final id = '${_next++}';
    final completer = Completer<Map<String, Object?>>();
    _pending[id] = completer;
    _socket.add(jsonEncode({'jsonrpc': '2.0', 'id': id, 'method': method, 'params': params}));
    return completer.future;
  }

  /// Closes the connection.
  Future<void> close() => _socket.close();
}

/// Runs `dart` with [arguments] in [directory], its streams being ours; its exit code.
Future<int> dart(List<String> arguments, Directory directory) =>
    command('dart', arguments, directory);

/// Runs [executable] with [arguments] in [directory], its streams being ours; its exit code.
Future<int> command(String executable, List<String> arguments, Directory directory) async {
  final process = await Process.start(
    executable,
    arguments,
    workingDirectory: directory.path,
    mode: ProcessStartMode.inheritStdio,
    runInShell: Platform.isWindows,
  );
  return process.exitCode;
}

/// The nearest directory at or above [start] that has a pubspec.yaml, or null.
Directory? packageDirectory(Directory start) {
  var directory = start;
  while (true) {
    if (File('${directory.path}${Platform.pathSeparator}pubspec.yaml').existsSync()) {
      return directory;
    }
    final parent = directory.parent;
    if (parent.path == directory.path) {
      return null;
    }
    directory = parent;
  }
}

/// Whether the pubspec.yaml of [package] lists `build_runner` among its `dev_dependencies`.
bool usesBuildRunner(Directory package) {
  final pubspec = File('${package.path}${Platform.pathSeparator}pubspec.yaml');
  return devDependsOnBuildRunner(pubspec.readAsStringSync());
}

/// Whether the pubspec.yaml of [package] depends on the Flutter SDK.
bool isFlutterPackage(Directory package) {
  final pubspec = File('${package.path}${Platform.pathSeparator}pubspec.yaml');
  return dependsOnFlutter(pubspec.readAsStringSync());
}

final _flutterKey = RegExp(r"""^['"]?flutter['"]?\s*:""");
final _sdkFlutter = RegExp(r"""^sdk\s*:\s*['"]?flutter['"]?\s*(#.*)?$""");

/// Whether [pubspec] has `flutter: sdk: flutter` in `dependencies` (read by lines, like
/// [devDependsOnBuildRunner]).
bool dependsOnFlutter(String pubspec) {
  var inDependencies = false;
  var inFlutter = false;
  int? indent;
  for (final line in LineSplitter.split(pubspec)) {
    final entry = line.trimLeft();
    if (entry.isEmpty || entry.startsWith('#')) {
      continue;
    }
    final depth = line.length - entry.length;
    if (depth == 0) {
      inDependencies = RegExp(r'^dependencies\s*:').hasMatch(line);
      inFlutter = false;
      indent = null;
    } else if (inDependencies) {
      indent ??= depth;
      if (depth == indent) {
        inFlutter = _flutterKey.hasMatch(entry);
      } else if (inFlutter && _sdkFlutter.hasMatch(entry)) {
        return true;
      }
    }
  }
  return false;
}

final _buildRunnerKey = RegExp(r"""^['"]?build_runner['"]?\s*:""");

/// Whether [pubspec] lists `build_runner` among its `dev_dependencies`. Read by lines, since the
/// tools bring no YAML parser: the key at the indentation of the section's first entry.
bool devDependsOnBuildRunner(String pubspec) {
  var inDevDependencies = false;
  int? indent;
  for (final line in LineSplitter.split(pubspec)) {
    final entry = line.trimLeft();
    if (entry.isEmpty || entry.startsWith('#')) {
      continue;
    }
    if (entry.length == line.length) {
      inDevDependencies = RegExp(r'^dev_dependencies\s*:').hasMatch(line);
      indent = null;
    } else if (inDevDependencies) {
      indent ??= line.length - entry.length;
      if (line.length - entry.length == indent && _buildRunnerKey.hasMatch(entry)) {
        return true;
      }
    }
  }
  return false;
}
