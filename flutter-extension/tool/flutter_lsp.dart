// A language server for Dart files, next to the Dart language server, with three small jobs:
//
//     dart flutter_lsp.dart
//
// - The snippets (snippets.json) come from here as completions, which, unlike Zed's snippets, can
//   carry edits that add the imports a snippet needs (`log` dart:developer, `scf`
//   flutter/material.dart, ...) when the file does not have them yet. They are offered for the
//   word being typed (`sc` offers `sc`, `scf`, ...), never after `{`, `(`, `.`, a space, ...
// - The `Flutter: run on <platform>` and `Flutter DevTools: <page>` tasks of the Dart language
//   (languages/dart/tasks.json) start `~/.zed-flutter/flutter_run.dart` and
//   `~/.zed-flutter/flutter_devtools.dart`: a task cannot name the extension's own directory, which
//   differs from one OS to the next. On start, this server copies them and what they import
//   (runnerFiles) from next to itself to ~/.zed-flutter/, when they differ.
// - For each project of the workspace that has `build_runner` in the `dev_dependencies` of its
//   pubspec.yaml (the root and the packages below it, down to [maxProjectDepth] levels: a
//   monorepo's `example/flutter_app` too), it runs
//   `dart run build_runner watch --delete-conflicting-outputs` while the server lives, and shows
//   its output in Zed's language server log. A pub workspace gets one watch for all its packages
//   (`--workspace`, build_runner 2.14 and later), run in its root, or in a member when the root
//   does not depend on build_runner itself. The projects are looked at again every few seconds:
//   a watch starts for a new one, stops for a removed one, and restarts when its pubspec.yaml,
//   pubspec.lock or package_config.json changed (a workspace turned into a single package, a
//   `pub get`), since build_runner reads them once. A watch that ends by itself (build_runner
//   stops it when another build_runner command, like `build`, takes its lock) starts again. `"initialization_options": {"build_runner": false}`
//   in the settings of `flutter-tools`, or FLUTTER_TOOLS_BUILD_RUNNER=off in the environment,
//   turns that off.
//
// It writes nothing into projects itself (build_runner writes the generated files).

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'devices.dart' show workspaceMembers;
import 'protocol.dart';

/// Where the tasks find the runner: the same path on every machine.
const runnerDir = '.zed-flutter';

/// The programs the tasks run, and what they import.
const runnerFiles = [
  'flutter_run.dart',
  'dart_run.dart',
  'devices.dart',
  'flutter_devtools.dart',
  'devtools.dart',
];

/// What build_runner is run with, in the package directory of each project that uses it.
const buildRunnerArguments = ['run', 'build_runner', 'watch', '--delete-conflicting-outputs'];

/// Imports that bring what an import of the key brings: flutter/foundation.dart (debugPrint) comes
/// with widgets.dart, which comes with material.dart and cupertino.dart.
const providedBy = {
  'package:flutter/foundation.dart': [
    'package:flutter/foundation.dart',
    'package:flutter/widgets.dart',
    'package:flutter/material.dart',
    'package:flutter/cupertino.dart',
  ],
  'package:flutter/widgets.dart': [
    'package:flutter/widgets.dart',
    'package:flutter/material.dart',
    'package:flutter/cupertino.dart',
  ],
};

Future<void> main() async {
  final here = File.fromUri(Platform.script).parent;
  final snippets = loadSnippets(File('${here.path}/snippets.json'));
  final documents = <String, String>{};
  final watches = BuildRunnerWatches(_send);
  var roots = <Directory>[];
  var watchBuilds = true;
  await for (final message in stdin.transform(DapDecoder())) {
    final id = message['id'];
    final params = (message['params'] as Map?)?.cast<String, Object?>() ?? const {};
    switch (message['method']) {
      case 'initialize':
        roots = workspaceRoots(params);
        watchBuilds = buildRunnerEnabled(params['initializationOptions'], Platform.environment);
        final log = <String>[];
        try {
          installRunner(here, log.add);
        } on Object catch (error) {
          log.add('Flutter Tools could not install the runner of the run tasks: $error');
        }
        _send({
          'id': id,
          'result': {
            'capabilities': {
              // The whole text on each change: the server reads only the imports of it.
              'textDocumentSync': 1,
              'completionProvider': <String, Object?>{},
            },
            'serverInfo': {'name': 'flutter-tools'},
          },
        });
        for (final line in log) {
          _send({
            'method': 'window/logMessage',
            'params': {'type': 1, 'message': line},
          });
        }
      case 'initialized':
        if (watchBuilds) {
          watches.follow(() => [for (final root in roots) ...buildRunnerProjects(root)]);
        }
      case 'textDocument/didOpen':
        final document = params['textDocument'] as Map;
        documents[document['uri'] as String] = document['text'] as String;
      case 'textDocument/didChange':
        final uri = (params['textDocument'] as Map)['uri'] as String;
        final changes = params['contentChanges'] as List;
        if (changes.isNotEmpty) {
          documents[uri] = (changes.last as Map)['text'] as String;
        }
      case 'textDocument/didClose':
        documents.remove((params['textDocument'] as Map)['uri']);
      case 'textDocument/completion':
        final uri = (params['textDocument'] as Map)['uri'] as String;
        final position = (params['position'] as Map?) ?? const {};
        // Zed asks with TriggerCharacter (2) after a character some server declared (`{`, `.`,
        // `(` of the Dart server) and with Invoked (1, the LSP default) for typed letters and
        // for ctrl-space (or whatever key runs editor::ShowCompletions).
        final context = params['context'] as Map?;
        _send({
          'id': id,
          'result': {
            'isIncomplete': false,
            'items': completions(
              snippets,
              documents[uri] ?? '',
              (position['line'] as num?)?.toInt() ?? -1,
              (position['character'] as num?)?.toInt() ?? 0,
              invoked: (context?['triggerKind'] as num?)?.toInt() != 2,
            ),
          },
        });
      case 'shutdown':
        watches.stop();
        _send({'id': id, 'result': null});
      case 'exit':
        watches.stop();
        exit(0);
      default:
        if (id != null && message['method'] != null) {
          _send({
            'id': id,
            'error': {'code': -32601, 'message': 'not supported'},
          });
        }
    }
  }
  // Zed went away without `exit`.
  watches.stop();
  exit(0);
}

void _send(Map<String, Object?> message) =>
    stdout.add(encodeMessage({'jsonrpc': '2.0', ...message}));

class Snippet {
  Snippet(this.name, this.prefix, this.body, this.description, this.imports);

  final String name;
  final String prefix;
  final String body;
  final String description;
  final List<String> imports;
}

List<Snippet> loadSnippets(File file) {
  final json = (jsonDecode(file.readAsStringSync()) as Map).cast<String, Map>();
  return [
    for (final MapEntry(key: name, value: s) in json.entries)
      Snippet(
        name,
        s['prefix'] as String,
        (s['body'] as List).join('\n'),
        s['description'] as String,
        [...?(s['imports'] as List?)?.cast<String>()],
      ),
  ];
}

final _wordEnd = RegExp(r'[A-Za-z0-9_$]+$');

/// The word that ends at [character] (UTF-16 units, as in the protocol) of [line] (from 0) of
/// [text]: what has been typed of an identifier, or '' after anything else (`{`, `.`, a space, the
/// start of the line) and outside the text.
String typedPrefix(String text, int line, int character) {
  final lines = LineSplitter.split(text).toList();
  if (line < 0 || line >= lines.length || character <= 0) {
    return '';
  }
  final end = character < lines[line].length ? character : lines[line].length;
  return _wordEnd.firstMatch(lines[line].substring(0, end))?.group(0) ?? '';
}

/// The snippets as completion items for a file with [text], at [character] of [line]: those whose
/// prefix starts with the word being typed there (any case). With no word there, all of them when
/// the completion was [invoked] (ctrl-space), none when a trigger character brought it, so that
/// nothing pops up after `{` or `(`. Each adds the imports it needs that the file has none that
/// brings.
List<Map<String, Object?>> completions(
  List<Snippet> snippets,
  String text,
  int line,
  int character, {
  bool invoked = true,
}) {
  final typed = typedPrefix(text, line, character).toLowerCase();
  if (typed.isEmpty && !invoked) {
    return const [];
  }
  return [
    for (final snippet in snippets)
      if (typed.isEmpty || snippet.prefix.toLowerCase().startsWith(typed))
        {
          'label': snippet.prefix,
          'filterText': snippet.prefix,
          'kind': 15, // Snippet
          'detail': snippet.name,
          'documentation': '${snippet.description}\n\n`${snippet.body}`',
          'insertTextFormat': 2, // Snippet
          'insertText': snippet.body,
          if (importEdits(text, snippet.imports) case final edits when edits.isNotEmpty)
            'additionalTextEdits': edits,
        },
  ];
}

/// The edits that add the imports of [libraries] that [text] lacks, the `dart:` ones first: edits
/// at the same place apply in their order.
List<Map<String, Object?>> importEdits(String text, List<String> libraries) => [
  for (final library in [
    ...libraries.where((l) => l.startsWith('dart:')),
    ...libraries.where((l) => !l.startsWith('dart:')),
  ])
    if (importEdit(text, library) case final edit?) edit,
];

final _import = RegExp(r'''^\s*import\s+['"]([^'"]+)['"]''');

/// The edit that adds `import '<library>';` to [text], or null when it has it (or one that brings
/// the same). A `dart:` import goes after the last `dart:` one, or before all the others (a blank
/// line apart, as `dart format` groups them); others after the last import; with no imports, after
/// a `library` directive, or at the top.
Map<String, Object?>? importEdit(String text, String library) {
  final lines = LineSplitter.split(text).toList();
  final imports = <int, String>{
    for (var i = 0; i < lines.length; i++)
      if (_import.firstMatch(lines[i]) case final m?) i: m.group(1)!,
  };
  final satisfying = providedBy[library] ?? [library];
  if (imports.values.any(satisfying.contains)) {
    return null;
  }
  var line = 0;
  var newText = "import '$library';\n";
  if (imports.isEmpty) {
    line = lines.indexWhere((l) => l.startsWith('library ')) + 1;
  } else if (library.startsWith('dart:')) {
    final dart = imports.keys.where((i) => imports[i]!.startsWith('dart:'));
    if (dart.isNotEmpty) {
      line = dart.last + 1;
    } else {
      line = imports.keys.first;
      newText += '\n';
    }
  } else {
    line = imports.keys.last + 1;
  }
  return {
    'range': {
      'start': {'line': line, 'character': 0},
      'end': {'line': line, 'character': 0},
    },
    'newText': newText,
  };
}

/// Copies the programs of the tasks next to this file to ~/.zed-flutter/, when they differ.
void installRunner(Directory source, void Function(String) log) {
  final home = Platform.environment['HOME'] ?? Platform.environment['USERPROFILE'];
  if (home == null) {
    log('Flutter Tools: no home directory, so the run tasks will not find their runner');
    return;
  }
  final target = Directory('$home/$runnerDir')..createSync(recursive: true);
  for (final name in runnerFiles) {
    final text = File('${source.path}/$name').readAsStringSync();
    final to = File('${target.path}/$name');
    if (!to.existsSync() || to.readAsStringSync() != text) {
      to.writeAsStringSync(text);
    }
  }
}

/// The folders of the workspace the client opened: its `workspaceFolders`, else its `rootUri`.
List<Directory> workspaceRoots(Map<String, Object?> initializeParams) {
  final uris = <String>[
    for (final folder in (initializeParams['workspaceFolders'] as List?) ?? const [])
      if (folder is Map && folder['uri'] is String) folder['uri'] as String,
  ];
  if (uris.isEmpty && initializeParams['rootUri'] is String) {
    uris.add(initializeParams['rootUri'] as String);
  }
  return [
    for (final uri in uris.toSet())
      if (Uri.tryParse(uri) case final parsed? when parsed.scheme == 'file')
        Directory.fromUri(parsed),
  ];
}

/// Whether to run build_runner: not when the `build_runner` initialization option is false (or
/// "off"), nor when FLUTTER_TOOLS_BUILD_RUNNER is "off" (or false, 0, no).
bool buildRunnerEnabled(Object? initializationOptions, Map<String, String> environment) {
  bool off(Object? value) =>
      value == false ||
      (value is String && const {'off', 'false', '0', 'no'}.contains(value.trim().toLowerCase()));
  final option = initializationOptions is Map ? initializationOptions['build_runner'] : null;
  return !off(option) && !off(environment['FLUTTER_TOOLS_BUILD_RUNNER']);
}

/// How deep below the root packages are looked for (`example/flutter_app` is 2).
const maxProjectDepth = 4;

/// Directories never looked into for packages: build output, other tools' dependencies.
const _skippedDirectories = {'build', 'node_modules', 'Pods', 'target'};

/// The package directories to run build_runner in: [root] and the packages below it, down to
/// [maxProjectDepth] levels (the members of a pub workspace, the packages of a monorepo, apps in
/// its `example/`), hidden directories and build output left out, whose pubspec.yaml has
/// `build_runner` in `dev_dependencies`. Of a pub workspace whose build_runner builds workspaces
/// (see [workspaceBuildRoot]), only the first: its watch builds them all.
List<Directory> buildRunnerProjects(Directory root) {
  final projects = <Directory>[];
  final workspaces = <String>{};
  for (final project in _packagesUsingBuildRunner(root)) {
    final workspace = workspaceBuildRoot(project);
    if (workspace == null || workspaces.add(workspace.absolute.path)) {
      projects.add(project);
    }
  }
  return projects;
}

/// The `dart` arguments of the watch in [project]: [buildRunnerArguments], with `--workspace` in
/// a pub workspace whose build_runner builds workspaces.
List<String> buildRunnerArgumentsFor(Directory project) => [
  ...buildRunnerArguments,
  if (workspaceBuildRoot(project) != null) '--workspace',
];

/// The root of the pub workspace [project] belongs to (itself, or the parent listing it in
/// `workspace:`), when its pubspec.lock has build_runner 2.14 or later, which builds a whole
/// workspace with `--workspace`; otherwise null, and each package is watched on its own.
Directory? workspaceBuildRoot(Directory project) {
  final path = _normalized(project);
  // The root may be any ancestor: `workspace: [example/app]` lists a member two levels down.
  for (var candidate = project; ; candidate = candidate.parent) {
    final members = workspaceMembers(candidate);
    final isRoot =
        members.isNotEmpty &&
        (_normalized(candidate) == path || members.any((member) => _normalized(member) == path));
    if (isRoot) {
      final version = _lockedVersion(candidate, 'build_runner');
      return version != null && _atLeast(version, const [2, 14]) ? candidate : null;
    }
    if (_normalized(candidate.parent) == _normalized(candidate)) {
      return null; // the file system's root
    }
  }
}

/// [directory] as an absolute path without `.`, `..` or a trailing separator, to compare.
String _normalized(Directory directory) =>
    directory.absolute.uri.normalizePath().toFilePath().replaceFirst(RegExp(r'[\\/]$'), '');

/// The version of [package] in [root]'s pubspec.lock, e.g. `2.17.1`.
String? _lockedVersion(Directory root, String package) {
  final String text;
  try {
    text = File('${root.path}/pubspec.lock').readAsStringSync();
  } on FileSystemException {
    return null;
  }
  var inPackage = false;
  for (final line in LineSplitter.split(text)) {
    if (RegExp(r'^  \S').hasMatch(line)) {
      inPackage = line.trim() == '$package:';
    } else if (inPackage) {
      final version = RegExp(r'''^    version:\s*['"]?([^'"\s]+)''').firstMatch(line);
      if (version != null) {
        return version.group(1);
      }
    }
  }
  return null;
}

bool _atLeast(String version, List<int> minimum) {
  final parts = version.split(RegExp(r'[.+-]')).map(int.tryParse).toList();
  for (var i = 0; i < minimum.length; i++) {
    final part = i < parts.length ? parts[i] ?? 0 : 0;
    if (part != minimum[i]) {
      return part > minimum[i];
    }
  }
  return true;
}

List<Directory> _packagesUsingBuildRunner(Directory root) {
  final packages = <Directory>[];
  void visit(Directory directory, int depth) {
    if (_readPubspec(directory) case final text? when devDependsOnBuildRunner(text)) {
      packages.add(directory);
    }
    if (depth == maxProjectDepth) {
      return;
    }
    final List<Directory> children;
    try {
      children = directory.listSync(followLinks: false).whereType<Directory>().where((child) {
        final name = _basename(child.path);
        return !name.startsWith('.') && !_skippedDirectories.contains(name);
      }).toList()..sort((a, b) => a.path.compareTo(b.path));
    } on FileSystemException {
      return; // a directory that cannot be listed has no packages for us
    }
    for (final child in children) {
      visit(child, depth + 1);
    }
  }

  visit(root, 0);
  return packages;
}

String? _readPubspec(Directory package) {
  try {
    return File('${package.path}/pubspec.yaml').readAsStringSync();
  } on FileSystemException {
    return null;
  }
}

String _basename(String path) => path.split(RegExp(r'[\\/]')).where((s) => s.isNotEmpty).last;

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

final _buildSummary = RegExp(r'\b(Succeeded|Failed) after\b');

/// The `build_runner watch` processes of a workspace, with their output in the client's log.
class BuildRunnerWatches {
  BuildRunnerWatches(this._send);

  final void Function(Map<String, Object?>) _send;

  /// The watches, by [_key]: their process (null while it starts), the [_stamp] of the files
  /// they were started after, and when they started.
  final _watches = <String, ({Process? process, String stamp, DateTime started})>{};

  /// How long changes count as the watch's own: `dart run` resolves the packages first when
  /// pubspec.lock is older than pubspec.yaml, which writes the files the stamp is made of.
  static Duration get settling => Duration(
    milliseconds:
        int.tryParse(Platform.environment['FLUTTER_TOOLS_BUILD_RUNNER_SETTLE_MS'] ?? '') ?? 10000,
  );

  /// A change seen by [sync], applied once the next sync sees the same (a `pub get` writes
  /// several files).
  String? _pending;
  Timer? _timer;
  var _stopped = false;

  /// How often [follow] looks at the projects again.
  static Duration get interval => Duration(
    milliseconds:
        int.tryParse(Platform.environment['FLUTTER_TOOLS_BUILD_RUNNER_POLL_MS'] ?? '') ?? 2000,
  );

  /// Runs the watches [projects] asks for, now and every [interval].
  void follow(List<Directory> Function() projects) {
    sync(projects(), immediately: true);
    _timer = Timer.periodic(interval, (_) => sync(projects()));
  }

  /// Starts a watch in each of [projects] that has none, stops those of projects no longer in
  /// it, and restarts those whose pubspec.yaml, pubspec.lock or package_config.json changed.
  /// Unless [immediately], a change waits for the next call that sees it unchanged.
  void sync(List<Directory> projects, {bool immediately = false}) {
    if (_stopped) {
      return;
    }
    final wanted = {
      for (final project in projects)
        if (buildRunnerArgumentsFor(project) case final arguments)
          _key(project, arguments): (project, arguments),
    };
    final stamps = {for (final MapEntry(:key, :value) in wanted.entries) key: _stamp(value.$1)};
    // Changes right after a start are the start's own (see [settling]): the new stamp is kept.
    final now = DateTime.now();
    for (final MapEntry(:key, value: stamp) in stamps.entries) {
      final watch = _watches[key];
      if (watch != null && watch.stamp != stamp && now.difference(watch.started) < settling) {
        _watches[key] = (process: watch.process, stamp: stamp, started: watch.started);
      }
    }
    final changed =
        wanted.length != _watches.length ||
        stamps.entries.any((e) => _watches[e.key]?.stamp != e.value);
    if (!changed) {
      _pending = null;
      return;
    }
    final state = stamps.entries.map((e) => '${e.key}=${e.value}').join('\n');
    if (!immediately && _pending != state) {
      _pending = state;
      return;
    }
    _pending = null;
    for (final key in [..._watches.keys]) {
      if (stamps[key] != _watches[key]!.stamp) {
        final process = _watches.remove(key)!.process;
        if (process != null) {
          killTree(process);
        }
      }
    }
    for (final MapEntry(:key, :value) in wanted.entries) {
      if (!_watches.containsKey(key)) {
        final stamp = stamps[key]!;
        final started = DateTime.now();
        _watches[key] = (process: null, stamp: stamp, started: started);
        _watch(key, started, value.$1, value.$2);
      }
    }
  }

  static String _key(Directory project, List<String> arguments) =>
      '${project.path}\u0000${arguments.join(' ')}';

  /// When [project]'s pubspec.yaml, pubspec.lock and package_config.json last changed.
  static String _stamp(Directory project) =>
      ['pubspec.yaml', 'pubspec.lock', '.dart_tool/package_config.json']
          .map((name) {
            try {
              return File('${project.path}/$name').lastModifiedSync().microsecondsSinceEpoch;
            } on FileSystemException {
              return 0;
            }
          })
          .join(',');

  Future<void> _watch(
    String key,
    DateTime started,
    Directory project,
    List<String> arguments,
  ) async {
    final name = _basename(project.path);
    void message(String method, int type, String text) => _send({
      'method': method,
      'params': {'type': type, 'message': 'build_runner $name: $text'},
    });
    final Process process;
    try {
      // The Dart that runs this server: the one of the Flutter SDK, or fvm's.
      process = await Process.start(
        Platform.resolvedExecutable,
        arguments,
        workingDirectory: project.path,
      );
    } on ProcessException catch (error) {
      message('window/logMessage', 1, 'could not start: ${error.message}');
      return;
    }
    // Stopped, or replaced by a newer watch, while starting.
    final watch = _watches[key];
    if (_stopped || watch == null || watch.process != null || watch.started != started) {
      killTree(process);
      return;
    }
    _watches[key] = (process: process, stamp: watch.stamp, started: started);
    // Whether build_runner gave its lock to another build_runner command (a `build` run by
    // hand or by a task): build_runner 2.17+ stops a watch for that.
    var yielded = false;
    void forward(Stream<List<int>> output, int type) {
      output
          .transform(const Utf8Decoder(allowMalformed: true))
          .transform(const LineSplitter())
          .listen((line) {
            if (line.trim().isEmpty) {
              return;
            }
            if (line.contains(_yieldedLock)) {
              yielded = true;
            }
            message('window/logMessage', type, line);
            if (_buildSummary.firstMatch(line) case final summary?) {
              message('window/showMessage', summary.group(1) == 'Failed' ? 1 : 3, line.trim());
            }
          });
    }

    forward(process.stdout, 3);
    forward(process.stderr, 2);
    process.exitCode.then((code) {
      // Not stopped by this server, and still the watch for its project: it ended by itself.
      final current = _watches[key]?.process == process;
      if (_stopped || !current) {
        return;
      }
      message('window/logMessage', code == 0 ? 3 : 1, 'exited with code $code');
      // A watch that gave way to another build_runner, or ran for a while, starts again with the
      // next sync: the new one waits for the other command's lock ("Waiting for
      // already-running build_runner"). One that fails right after starting stays stopped until
      // its pubspec changes, so a broken setup does not restart over and over.
      if (yielded || DateTime.now().difference(started) >= settling) {
        _watches.remove(key);
        message('window/logMessage', 3, 'restarting');
      }
    });
  }

  /// Stops the watches, for good.
  void stop() {
    _stopped = true;
    _timer?.cancel();
    for (final watch in _watches.values) {
      if (watch.process case final process?) {
        killTree(process);
      }
    }
    _watches.clear();
  }
}

/// What build_runner prints when it stops a watch for another build_runner command.
const _yieldedLock = 'Exiting as requested by another build_runner process';

/// Kills [process] and what it started: `dart run` runs the program in a process of its own, and
/// build_runner the build script in another, which would run on without it.
void killTree(Process process) {
  try {
    if (Platform.isWindows) {
      Process.runSync('taskkill', ['/T', '/F', '/PID', '${process.pid}']);
      return;
    }
    final table = Process.runSync('ps', ['-A', '-o', 'pid=,ppid=']).stdout as String;
    for (final pid in [...descendants(process.pid, table), process.pid]) {
      Process.killPid(pid);
    }
  } on ProcessException {
    process.kill();
  }
}

/// The pids under [root] in a `ps -o pid=,ppid=` [table], children and their children.
List<int> descendants(int root, String table) {
  final children = <int, List<int>>{};
  for (final line in LineSplitter.split(table)) {
    final columns = line.trim().split(RegExp(r'\s+'));
    final pid = columns.length == 2 ? int.tryParse(columns[0]) : null;
    final parent = columns.length == 2 ? int.tryParse(columns[1]) : null;
    if (pid != null && parent != null && pid != root) {
      children.putIfAbsent(parent, () => []).add(pid);
    }
  }
  final found = <int>[];
  for (final pending = [root]; pending.isNotEmpty;) {
    for (final child in children[pending.removeLast()] ?? const <int>[]) {
      found.add(child);
      pending.add(child);
    }
  }
  return found;
}
