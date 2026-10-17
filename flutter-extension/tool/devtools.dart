// Flutter DevTools for apps running under the `Flutter` debug adapter, shared by the proxy (which
// serves DevTools for its app and opens in Zed the code of the widget selected in it) and
// flutter_devtools.dart (which opens a DevTools page in a window of its own).
//
// The proxy writes a session file, ~/.zed-flutter/sessions/<its pid>.json, for each app it runs:
// the app's project directory, its VM service and the address of its DevTools. A task finds the
// app of its project there. Nothing goes into the project.

import 'dart:async';
import 'dart:convert';
import 'dart:io';

/// The DevTools pages the tasks open, with their names.
const pages = {
  'inspector': 'Widget Inspector',
  'performance': 'Performance',
  'cpu-profiler': 'CPU Profiler',
  'memory': 'Memory',
  'network': 'Network',
  'logging': 'Logging',
};

/// ~/.zed-flutter: where the tasks find the runner and the sessions, the same path on every
/// machine.
Directory? toolsHome() {
  final home = Platform.environment['HOME'] ?? Platform.environment['USERPROFILE'];
  return home == null ? null : Directory('$home${Platform.pathSeparator}.zed-flutter');
}

Directory? sessionsDir() {
  final home = toolsHome();
  return home == null ? null : Directory('${home.path}${Platform.pathSeparator}sessions');
}

/// An app running under the debug adapter, as its session file has it.
class Session {
  Session({required this.cwd, required this.vmServiceUri, required this.devTools, this.file});

  factory Session.fromJson(Map<String, Object?> json, File file) => Session(
    cwd: json['cwd'] as String,
    vmServiceUri: json['vmServiceUri'] as String,
    devTools: json['devTools'] as String,
    file: file,
  );

  /// The project directory.
  final String cwd;
  final String vmServiceUri;

  /// The address DevTools is served on, ending in `/`.
  final String devTools;
  final File? file;

  Map<String, Object?> toJson() => {'cwd': cwd, 'vmServiceUri': vmServiceUri, 'devTools': devTools};

  /// The address of DevTools' [page], connected to this app.
  String pageUrl(String page) => '$devTools$page?uri=${Uri.encodeComponent(vmServiceUri)}';
}

/// A DevTools server, started by [DevToolsServer.start].
class DevToolsServer {
  DevToolsServer._(this._process, this.address);

  final Process _process;

  /// Where DevTools is served, ending in `/`.
  final String address;

  /// Starts a DevTools server with the Dart that runs this program (the Flutter SDK's), or the
  /// command of `FLUTTER_TOOLS_DEVTOOLS` (a JSON list), which must print the `server.started`
  /// event of `dart devtools --machine`.
  static Future<DevToolsServer> start() async {
    final override = Platform.environment['FLUTTER_TOOLS_DEVTOOLS'];
    final command = override != null
        ? (jsonDecode(override) as List).cast<String>()
        : [Platform.resolvedExecutable, 'devtools', '--machine', '--no-launch-browser', '--port=0'];
    final process = await Process.start(command.first, command.sublist(1));
    process.stderr.drain<void>();
    final started = Completer<String>();
    final lines = process.stdout.transform(utf8.decoder).transform(const LineSplitter());
    lines.listen(
      (line) {
        try {
          final event = jsonDecode(line);
          if (event is Map && event['event'] == 'server.started' && !started.isCompleted) {
            final params = event['params'] as Map;
            started.complete('http://${params['host']}:${params['port']}/');
          }
        } on FormatException {
          // Not an event.
        }
      },
      onDone: () {
        if (!started.isCompleted) {
          started.completeError(StateError('DevTools quit before it started'));
        }
      },
    );
    try {
      return DevToolsServer._(process, await started.future.timeout(const Duration(minutes: 1)));
    } on Object {
      process.kill();
      rethrow;
    }
  }

  void stop() => _process.kill();
}

/// The session file of the process [pid].
File? sessionFile(int pid) {
  final dir = sessionsDir();
  return dir == null ? null : File('${dir.path}${Platform.pathSeparator}$pid.json');
}

/// The sessions whose DevTools still answers, the latest first; the files of the others (an
/// adapter that did not get to remove its own) are deleted.
Future<List<Session>> liveSessions() async {
  final dir = sessionsDir();
  if (dir == null || !dir.existsSync()) {
    return [];
  }
  final files = dir.listSync().whereType<File>().where((f) => f.path.endsWith('.json')).toList()
    ..sort((a, b) => b.lastModifiedSync().compareTo(a.lastModifiedSync()));
  final sessions = <Session>[];
  for (final file in files) {
    Session? session;
    try {
      session = Session.fromJson((jsonDecode(file.readAsStringSync()) as Map).cast(), file);
    } on Object {
      // Half written, or not ours.
    }
    if (session != null && await _answers(session.devTools)) {
      sessions.add(session);
    } else {
      _delete(file);
    }
  }
  return sessions;
}

Future<bool> _answers(String url) async {
  final client = HttpClient()..connectionTimeout = const Duration(seconds: 2);
  try {
    final response = await (await client.getUrl(Uri.parse(url)))
        .close()
        .timeout(const Duration(seconds: 5));
    await response.drain<void>();
    return true;
  } on Object {
    return false;
  } finally {
    client.close(force: true);
  }
}

void _delete(File file) {
  try {
    file.deleteSync();
  } on FileSystemException {
    // Gone already.
  }
}

/// The session of the project at [root], or of the project [root] is in; otherwise the latest.
Session? sessionFor(List<Session> sessions, String? root) {
  if (root != null) {
    final dir = _normalize(root);
    for (final session in sessions) {
      final cwd = _normalize(session.cwd);
      if (cwd == dir || dir.startsWith('$cwd/') || cwd.startsWith('$dir/')) {
        return session;
      }
    }
  }
  return sessions.isEmpty ? null : sessions.first;
}

String _normalize(String path) {
  var p = path.replaceAll('\\', '/');
  while (p.length > 1 && p.endsWith('/')) {
    p = p.substring(0, p.length - 1);
  }
  return Platform.isWindows || Platform.isMacOS ? p.toLowerCase() : p;
}

/// Opens [url] in a window of its own: Chrome, Edge or Chromium in app mode (no tabs, no address
/// bar), otherwise the default browser. `FLUTTER_TOOLS_BROWSER` (a JSON list) overrides the
/// command; the URL is added to it.
Future<void> openWindow(String url) async {
  final override = Platform.environment['FLUTTER_TOOLS_BROWSER'];
  final List<String> command;
  if (override != null) {
    command = [...(jsonDecode(override) as List).cast<String>(), url];
  } else {
    command = _appWindow(url) ?? _defaultBrowser(url);
  }
  await Process.start(command.first, command.sublist(1), mode: ProcessStartMode.detached);
}

List<String>? _appWindow(String url) {
  final app = '--app=$url';
  if (Platform.isMacOS) {
    final home = Platform.environment['HOME'] ?? '';
    for (final name in ['Google Chrome', 'Microsoft Edge', 'Chromium', 'Brave Browser']) {
      for (final dir in ['/Applications', '$home/Applications']) {
        if (Directory('$dir/$name.app').existsSync()) {
          return ['open', '-na', '$dir/$name.app', '--args', app];
        }
      }
    }
    return null;
  }
  if (Platform.isWindows) {
    final env = Platform.environment;
    final candidates = [
      for (final base in [env['ProgramFiles'], env['ProgramFiles(x86)'], env['LOCALAPPDATA']])
        if (base != null) ...[
          '$base\\Google\\Chrome\\Application\\chrome.exe',
          '$base\\Microsoft\\Edge\\Application\\msedge.exe',
        ],
    ];
    for (final exe in candidates) {
      if (File(exe).existsSync()) {
        return [exe, app];
      }
    }
    return null;
  }
  for (final name in [
    'google-chrome',
    'google-chrome-stable',
    'chromium',
    'chromium-browser',
    'microsoft-edge',
  ]) {
    final exe = which(name);
    if (exe != null) {
      return [exe, app];
    }
  }
  return null;
}

List<String> _defaultBrowser(String url) {
  if (Platform.isMacOS) {
    return ['open', url];
  }
  if (Platform.isWindows) {
    return ['cmd', '/c', 'start', '', url];
  }
  return ['xdg-open', url];
}

/// The command that opens a file in Zed: `FLUTTER_TOOLS_ZED` (a JSON list), else Zed's command
/// line tool, from PATH or where Zed installs it.
List<String>? zedCommand() {
  final override = Platform.environment['FLUTTER_TOOLS_ZED'];
  if (override != null) {
    return (jsonDecode(override) as List).cast<String>();
  }
  for (final name in ['zed', 'zeditor', 'zedit']) {
    final exe = which(name);
    if (exe != null) {
      return [exe];
    }
  }
  final env = Platform.environment;
  final home = env['HOME'] ?? env['USERPROFILE'] ?? '';
  final candidates = [
    if (Platform.isMacOS)
      for (final dir in ['/Applications', '$home/Applications'])
        for (final app in ['Zed', 'Zed Preview']) '$dir/$app.app/Contents/MacOS/cli',
    if (Platform.isLinux) ...['$home/.local/bin/zed', '$home/.local/zed.app/bin/zed'],
    if (Platform.isWindows && env['LOCALAPPDATA'] != null)
      '${env['LOCALAPPDATA']}\\Programs\\Zed\\bin\\zed.exe',
  ];
  for (final exe in candidates) {
    if (File(exe).existsSync()) {
      return [exe];
    }
  }
  return null;
}

/// Opens [path] in Zed at [line] and [column] (1-based); false when Zed's command line tool is
/// nowhere to be found.
Future<bool> openInZed(String path, int? line, int? column) async {
  final zed = zedCommand();
  if (zed == null) {
    return false;
  }
  final at = line == null ? path : '$path:$line${column == null ? '' : ':$column'}';
  await Process.start(
    zed.first,
    [...zed.sublist(1), at],
    mode: ProcessStartMode.detached,
    runInShell: Platform.isWindows && !zed.first.toLowerCase().endsWith('.exe'),
  );
  return true;
}

/// The path of [name] in PATH, as a shell would find it.
String? which(String name) {
  final separator = Platform.isWindows ? ';' : ':';
  final extensions = Platform.isWindows
      ? (Platform.environment['PATHEXT'] ?? '.EXE;.CMD;.BAT').split(';')
      : [''];
  for (final dir in (Platform.environment['PATH'] ?? '').split(separator)) {
    if (dir.isEmpty) {
      continue;
    }
    for (final extension in extensions) {
      final file = File('$dir${Platform.pathSeparator}$name$extension');
      if (file.existsSync()) {
        return file.path;
      }
    }
  }
  return null;
}
