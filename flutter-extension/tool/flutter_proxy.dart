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
// Once the app runs, it also serves Flutter DevTools for it, and records where in a session file
// (see devtools.dart) for the `Flutter: DevTools ...` tasks; `"devTools": false` turns that off.
// When a widget is selected, in DevTools' inspector or in the app, Flutter sends a `navigate` tool
// event with its place in the code, which this opens in Zed.
//
// And the debugger's Stop quits the app: Flutter's adapter answers a `disconnect` by killing
// `flutter run`, which leaves a desktop app running, so a `terminate` goes first.
//
// Only `dart:` libraries are used, so that the Dart SDK alone runs this file.

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'devices.dart';
import 'devtools.dart';
import 'protocol.dart';

/// How long file changes are gathered into one reload: saving several files, or an editor
/// writing a file in more than one step, reloads once.
const debounce = Duration(milliseconds: 200);

/// Sequence numbers of the requests this proxy makes, far from the editor's own.
const firstOwnSeq = 1 << 30;

/// How long the editor's `disconnect` waits for the app to stop (see [Proxy._terminateFirst]).
const terminateTimeout = Duration(seconds: 10);

Future<void> main(List<String> args) async {
  if (args.isEmpty) {
    stderr.writeln('usage: dart flutter_proxy.dart <flutter> [args...] debug_adapter');
    exit(64);
  }
  final adapter = await Process.start(args.first, args.sublist(1), runInShell: Platform.isWindows);
  // `flutter debug_adapter` or `fvm flutter debug_adapter`: Flutter is the command before it.
  final flutter = args.sublist(0, args.lastIndexOf('debug_adapter').clamp(1, args.length));
  final proxy = Proxy(adapter.stdin, stdout, flutter);
  adapter.stderr.listen(stderr.add);
  final fromAdapter = adapter.stdout.transform(DapDecoder()).listen(proxy.fromAdapter);
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

  /// Messages from the editor held while a launch waits for its device, or a disconnect for the
  /// app to stop.
  List<Map<String, Object?>>? _held;
  int _seq = firstOwnSeq;

  /// The commands of this proxy's requests the adapter has yet to answer, by sequence number.
  final _own = <int, String>{};

  /// Whether the session launched the app (rather than attached to it), and whether the app was
  /// asked to stop, or has.
  bool _launched = false;
  bool _terminating = false;
  Timer? _terminateTimer;
  String? _root;
  bool _reloadOnSave = true;
  bool _devTools = true;
  bool _started = false;

  /// Whether DevTools was asked for, and whether the app has ended since: DevTools starts once,
  /// and not after the app is gone.
  bool _devToolsRequested = false;
  bool _ended = false;
  DevToolsServer? _devToolsServer;
  File? _session;
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
      _launched = message['command'] == 'launch';
      var arguments = message['arguments'];
      if (arguments is Map && message['command'] == 'launch') {
        // In a monorepo or a pub workspace the scenario's cwd (the worktree root by default) is
        // not the app's folder: Flutter is given the app under it.
        final cwd = arguments['cwd'];
        if (cwd is String) {
          try {
            final app = flutterApp(Directory(cwd)).path;
            if (app != cwd) {
              arguments = {...arguments, 'cwd': app};
              message = {...message, 'arguments': arguments};
            }
          } on DeviceException catch (error) {
            _send(_client, {
              'seq': 0,
              'type': 'response',
              'request_seq': message['seq'],
              'command': 'launch',
              'success': false,
              'message': '$error',
            });
            return;
          }
        }
      }
      if (arguments is Map) {
        _root = arguments['cwd'] as String?;
        _reloadOnSave = arguments['hotReloadOnSave'] != false;
        _devTools = arguments['devTools'] != false;
        final target = arguments['target'];
        final toolArgs = (arguments['toolArgs'] as List? ?? const []).cast<String>();
        final hasDevice = toolArgs.any((a) => a == '-d' || a.startsWith('--device-id'));
        if (target is String && !hasDevice && message['command'] == 'launch') {
          _held = [];
          _launchOn(target, message, arguments.cast<String, Object?>());
          return;
        }
      }
    }
    if (message['type'] == 'request') {
      final arguments = message['arguments'];
      final endsApp = arguments is! Map || arguments['terminateDebuggee'] != false;
      switch (message['command']) {
        case 'terminate':
          _terminating = true;
        case 'disconnect' when _launched && !_terminating && endsApp:
          _terminateFirst(message);
          return;
      }
    }
    _send(_adapter, message);
  }

  /// Flutter's adapter answers `disconnect` by killing `flutter run`, which leaves a desktop app
  /// running; its `terminate` stops the app (`app.stop`), as VS Code's Stop does. So a
  /// [disconnect] that ends a launched app goes after a `terminate`, once the app has stopped (or
  /// [terminateTimeout] has passed), and what the editor sends meanwhile after it.
  void _terminateFirst(Map<String, Object?> disconnect) {
    _terminating = true;
    _held = [disconnect];
    _terminateTimer = Timer(terminateTimeout, _releaseHeld);
    _request('terminate', const {});
  }

  void _releaseHeld() {
    _terminateTimer?.cancel();
    final held = _held;
    if (held != null) {
      _held = null;
      held.forEach(fromClient);
    }
  }

  /// Sends [launch] once [target] has a device, then what the editor sent meanwhile.
  Future<void> _launchOn(
    String target,
    Map<String, Object?> launch,
    Map<String, Object?> arguments,
  ) async {
    try {
      final device = await resolveDevice(target, _flutter, (line) => _output('$line\n'));
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
    _releaseHeld();
  }

  void fromAdapter(Map<String, Object?> message) {
    if (message['type'] == 'response') {
      switch (_own.remove(message['request_seq'])) {
        case 'hotReload':
          if (message['success'] == false) {
            _output('Hot reload failed: ${message['message']}\n');
          }
          return;
        case 'terminate':
          _releaseHeld();
          return;
      }
    }
    if (message['type'] == 'event') {
      final body = message['body'];
      switch (message['event']) {
        case 'flutter.appStarted':
          _started = true;
          _watchFiles();
        case 'dart.debuggerUris' when body is Map && body['vmServiceUri'] is String:
          _serveDevTools(body['vmServiceUri'] as String);
        case 'dart.toolEvent' when body is Map && body['kind'] == 'navigate':
          _navigate(body['data']);
        case 'terminated' || 'exited':
          _terminating = true;
          _started = false;
          _stopWatching();
          _stopDevTools();
      }
    }
    _send(_client, message);
  }

  Future<void> close() async {
    _terminateTimer?.cancel();
    _stopWatching();
    _stopDevTools();
    await _client.flush();
  }

  /// Serves DevTools for the app at [vmServiceUri], and records it in this adapter's session file.
  Future<void> _serveDevTools(String vmServiceUri) async {
    final root = _root;
    if (!_devTools || root == null || _devToolsRequested) {
      return;
    }
    _devToolsRequested = true;
    try {
      final server = await DevToolsServer.start();
      if (_ended) {
        server.stop();
        return;
      }
      _devToolsServer = server;
      final session = Session(cwd: root, vmServiceUri: vmServiceUri, devTools: server.address);
      final file = _session = sessionFile(pid);
      if (file != null) {
        file.parent.createSync(recursive: true);
        file.writeAsStringSync(jsonEncode(session.toJson()));
      }
      _output('Flutter DevTools: ${session.pageUrl('inspector')}\n');
    } on Object catch (error) {
      _output('Flutter DevTools did not start: $error\n');
    }
  }

  void _stopDevTools() {
    _ended = true;
    _devToolsServer?.stop();
    _devToolsServer = null;
    try {
      _session?.deleteSync();
    } on FileSystemException {
      // Gone already.
    }
    _session = null;
  }

  /// Opens in Zed the place of a `navigate` tool event: a widget selected in the inspector.
  Future<void> _navigate(Object? data) async {
    if (data is! Map) {
      return;
    }
    final uri = Uri.tryParse('${data['resolvedFileUri'] ?? data['fileUri']}');
    if (uri == null || !uri.isScheme('file')) {
      return;
    }
    final line = data['line'];
    final column = data['column'];
    try {
      if (!await openInZed(
        uri.toFilePath(),
        line is int ? line : null,
        column is int ? column : null,
      )) {
        _output(
          'Install Zed\'s command line tool (cli: install) to open selected widgets in Zed.\n',
        );
      }
    } on Object catch (error) {
      _output('Could not open ${uri.toFilePath()} in Zed: $error\n');
    }
  }

  void _watchFiles() {
    final root = _root;
    if (!_reloadOnSave || root == null || _watch != null) {
      return;
    }
    final lib = Directory('$root${Platform.pathSeparator}lib');
    final dir = lib.existsSync() ? lib : Directory(root);
    _watch = dir.watch(recursive: true).listen((event) {
      if (event.path.endsWith('.dart')) {
        _pending?.cancel();
        _pending = Timer(debounce, _reload);
      }
    }, onError: (Object error) => _output('Not watching ${dir.path} for hot reload: $error\n'));
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
    _request('hotReload', {'reason': 'save'});
  }

  /// Sends the adapter a request of this proxy's own, whose response the editor will not see.
  void _request(String command, Map<String, Object?> arguments) {
    final seq = _seq++;
    _own[seq] = command;
    _send(_adapter, {'seq': seq, 'type': 'request', 'command': command, 'arguments': arguments});
  }

  void _output(String text) {
    _send(_client, {
      'seq': 0,
      'type': 'event',
      'event': 'output',
      'body': {'category': 'console', 'output': text},
    });
  }

  void _send(IOSink sink, Map<String, Object?> message) => sink.add(encodeMessage(message));
}
