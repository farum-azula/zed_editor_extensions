// A debug adapter for Flutter that reloads the app when its code changes.
//
//     dart flutter_proxy.dart <flutter> [args...] debug_adapter
//
// Starts Flutter's own debug adapter (`flutter debug_adapter`) and passes every Debug Adapter
// Protocol message through between it and the editor, unchanged: breakpoints, stepping,
// variables, the console and Restart (a hot restart) are Flutter's. On top of that, once the app
// has started, a change to a `.dart` file under the project's `lib/` (the editor saving it) sends
// Flutter the `hotReload` request that VS Code sends on save. Its response is answered here and
// does not reach the editor, which never sent that request.
//
// The launch configuration may set `"hotReloadOnSave": false` to turn that off, and `"target"` (one
// of ios-simulator, android, web, macos, windows, linux) to run on a device of that kind: one is
// picked, or a simulator or an emulator started, before Flutter gets the launch request, as
// `-d <device>` in its `toolArgs`. Messages from the editor wait meanwhile, so their order holds.
//
// Only `dart:` libraries are used, so that the Dart SDK alone runs this file.

import 'dart:async';
import 'dart:io';

import 'devices.dart';
import 'protocol.dart';

/// How long file changes are gathered into one reload: saving several files, or an editor
/// writing a file in more than one step, reloads once.
const debounce = Duration(milliseconds: 200);

/// Sequence numbers of the requests this proxy makes, far from the editor's own.
const firstOwnSeq = 1 << 30;

Future<void> main(List<String> args) async {
  if (args.isEmpty) {
    stderr.writeln(
      'usage: dart flutter_proxy.dart <flutter> [args...] debug_adapter',
    );
    exit(64);
  }
  final adapter = await Process.start(
    args.first,
    args.sublist(1),
    runInShell: Platform.isWindows,
  );
  // `flutter debug_adapter` or `fvm flutter debug_adapter`: Flutter is the command before it.
  final flutter = args.sublist(
    0,
    args.lastIndexOf('debug_adapter').clamp(1, args.length),
  );
  final proxy = Proxy(adapter.stdin, stdout, flutter);
  adapter.stderr.listen(stderr.add);
  final fromAdapter = adapter.stdout
      .transform(DapDecoder())
      .listen(proxy.fromAdapter);
  stdin
      .transform(DapDecoder())
      .listen(
        proxy.fromClient,
        // The editor is gone: so is the session.
        onDone: () => adapter.kill(),
      );
  final code = await adapter.exitCode;
  await fromAdapter.asFuture<void>();
  await proxy.close();
  exit(code);
}

class Proxy {
  Proxy(this._adapter, this._client, this._flutter);

  final IOSink _adapter;
  final IOSink _client;
  final List<String> _flutter;

  /// Messages from the editor held while a launch waits for its device.
  List<Map<String, Object?>>? _held;
  int _seq = firstOwnSeq;
  final _own = <int>{};
  String? _root;
  bool _reloadOnSave = true;
  bool _started = false;
  StreamSubscription<FileSystemEvent>? _watch;
  Timer? _pending;

  void fromClient(Map<String, Object?> message) {
    final held = _held;
    if (held != null) {
      held.add(message);
      return;
    }
    if (message['type'] == 'request' &&
        (message['command'] == 'launch' || message['command'] == 'attach')) {
      final arguments = message['arguments'];
      if (arguments is Map) {
        _root = arguments['cwd'] as String?;
        _reloadOnSave = arguments['hotReloadOnSave'] != false;
        final target = arguments['target'];
        final toolArgs = (arguments['toolArgs'] as List? ?? const [])
            .cast<String>();
        final hasDevice = toolArgs.any(
          (a) => a == '-d' || a.startsWith('--device-id'),
        );
        if (target is String && !hasDevice && message['command'] == 'launch') {
          _held = [];
          _launchOn(target, message, arguments.cast<String, Object?>());
          return;
        }
      }
    }
    _send(_adapter, message);
  }

  /// Sends [launch] once [target] has a device, then what the editor sent meanwhile.
  Future<void> _launchOn(
    String target,
    Map<String, Object?> launch,
    Map<String, Object?> arguments,
  ) async {
    try {
      final device = await resolveDevice(
        target,
        _flutter,
        (line) => _output('$line\n'),
      );
      final toolArgs = [
        ...(arguments['toolArgs'] as List? ?? const []).cast<String>(),
        '-d',
        device,
      ];
      _send(_adapter, {
        ...launch,
        'arguments': {...arguments, 'toolArgs': toolArgs},
      });
    } on Object catch (error) {
      _send(_client, {
        'seq': 0,
        'type': 'response',
        'request_seq': launch['seq'],
        'command': 'launch',
        'success': false,
        'message': '$error',
      });
    }
    final held = _held!;
    _held = null;
    held.forEach(fromClient);
  }

  void fromAdapter(Map<String, Object?> message) {
    if (message['type'] == 'response' && _own.remove(message['request_seq'])) {
      if (message['success'] == false) {
        _output('Hot reload failed: ${message['message']}\n');
      }
      return;
    }
    if (message['type'] == 'event') {
      switch (message['event']) {
        case 'flutter.appStarted':
          _started = true;
          _watchFiles();
        case 'terminated' || 'exited':
          _started = false;
          _stopWatching();
      }
    }
    _send(_client, message);
  }

  Future<void> close() async {
    _stopWatching();
    await _client.flush();
  }

  void _watchFiles() {
    final root = _root;
    if (!_reloadOnSave || root == null || _watch != null) {
      return;
    }
    final lib = Directory('$root${Platform.pathSeparator}lib');
    final dir = lib.existsSync() ? lib : Directory(root);
    _watch = dir.watch(recursive: true).listen(
      (event) {
        if (event.path.endsWith('.dart')) {
          _pending?.cancel();
          _pending = Timer(debounce, _reload);
        }
      },
      onError: (Object error) =>
          _output('Not watching ${dir.path} for hot reload: $error\n'),
    );
  }

  void _stopWatching() {
    _pending?.cancel();
    _watch?.cancel();
    _watch = null;
  }

  void _reload() {
    if (!_started) {
      return;
    }
    final seq = _seq++;
    _own.add(seq);
    _send(_adapter, {
      'seq': seq,
      'type': 'request',
      'command': 'hotReload',
      'arguments': {'reason': 'save'},
    });
  }

  void _output(String text) {
    _send(_client, {
      'seq': 0,
      'type': 'event',
      'event': 'output',
      'body': {'category': 'console', 'output': text},
    });
  }

  void _send(IOSink sink, Map<String, Object?> message) =>
      sink.add(encodeMessage(message));
}
