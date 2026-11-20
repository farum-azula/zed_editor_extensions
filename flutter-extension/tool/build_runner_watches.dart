// The `build_runner watch` processes the language server keeps running (flutter_lsp.dart).
//
// Which targets get a watch is the [WatchScope] of the settings: by default one, the target of
// the file saved last, so that a monorepo does not run a watch per package (each rebuilding on
// every change of a package they share, and the analysis server re-reading all they write).
// A watch starts at most every [WatchTimings.stagger], restarts when its pubspec.yaml,
// pubspec.lock or package_config.json changed (build_runner reads them once), starts again when
// it ended by itself (another build_runner command took its lock), and is stopped when it keeps
// its lock too long after such a request. Its output goes to the server's log in the editor; the
// Succeeded / Failed lines of builds that end together make one notification.

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'build_runner.dart';
import 'lsp_server.dart';
import 'paths.dart';
import 'pubspec.dart';

/// Which targets get a watch: the `build_runner` initialization option of flutter-tools
/// (`"active"`, `"open"`, `"all"`, `"off"`, or true / false), else FLUTTER_TOOLS_BUILD_RUNNER; off
/// when either is off.
enum WatchScope {
  /// One target at a time: that of the file saved last (before any save, of the first file
  /// opened), until [WatchTimings.linger] after its last file closed.
  active,

  /// The targets that build a file open in the editor, until [WatchTimings.linger] after the last
  /// one closed.
  open,

  /// Every target under the editor's folders.
  all,

  /// None.
  off;

  /// The scope the settings ask for; [active] when they say nothing.
  static WatchScope from(Object? initializationOptions, Map<String, String> environment) {
    final option = initializationOptions is Map
        ? _parse(initializationOptions['build_runner'])
        : null;
    final variable = _parse(environment['FLUTTER_TOOLS_BUILD_RUNNER']);
    if (option == off || variable == off) {
      return off;
    }
    return option ?? variable ?? active;
  }

  static WatchScope? _parse(Object? value) => switch (value) {
    false => off,
    true => active,
    final String text => switch (text.trim().toLowerCase()) {
      'off' || 'false' || '0' || 'no' => off,
      'all' => all,
      'open' => open,
      'active' || 'on' || 'true' || '1' || 'yes' => active,
      _ => null,
    },
    _ => null,
  };
}

/// How long the watches wait for what. Each can be set in milliseconds by an environment variable,
/// `FLUTTER_TOOLS_BUILD_RUNNER_<NAME>_MS` (the tests make them short).
final class WatchTimings {
  WatchTimings.from(Map<String, String> environment)
    : poll = _duration(environment, 'POLL', 2000),
      settle = _duration(environment, 'SETTLE', 10000),
      yieldLock = _duration(environment, 'YIELD', 30000),
      linger = _duration(environment, 'LINGER', 600000),
      stagger = _duration(environment, 'STAGGER', 3000),
      summary = _duration(environment, 'SUMMARY', 1500),
      rescan = _duration(environment, 'RESCAN', 30000);

  /// How often the targets and their files are looked at.
  final Duration poll;

  /// How long changes of a target's configuration count as its start's own: `dart run` resolves
  /// the packages first when pubspec.lock is older than pubspec.yaml, which writes those files.
  final Duration settle;

  /// How long a watch may keep its lock after another build_runner command asked for it
  /// (build_runner 2.17 finishes the current build, then exits) before it is stopped.
  final Duration yieldLock;

  /// How long the watch of a package lives on after its last open file was closed.
  final Duration linger;

  /// The least time between two starts: several watches start one after another.
  final Duration stagger;

  /// How long the Succeeded / Failed lines are gathered into one notification.
  final Duration summary;

  /// How often what was learned about the packages is dropped and looked at again, besides when
  /// the editor reports a change of a pubspec.yaml.
  final Duration rescan;

  static Duration _duration(Map<String, String> environment, String name, int milliseconds) =>
      Duration(
        milliseconds:
            int.tryParse(environment['FLUTTER_TOOLS_BUILD_RUNNER_${name}_MS'] ?? '') ??
            milliseconds,
      );
}

/// Which targets should have a watch.
abstract interface class WatchedTargets {
  /// The targets that should have a watch at [now].
  List<BuildRunnerTarget> current(DateTime now);

  /// Drops what was learned from the file system (which package a file is in, what builds a
  /// package): a pubspec.yaml or pubspec.lock changed.
  void forget();
}

/// [WatchScope.all]: every target under the editor's [folders].
final class TargetsUnderFolders implements WatchedTargets {
  TargetsUnderFolders(this.folders);

  final List<Directory> folders;
  List<BuildRunnerTarget>? _targets;

  @override
  List<BuildRunnerTarget> current(DateTime now) => _targets ??= [
    ...{
      for (final folder in folders)
        for (final target in BuildRunnerTarget.under(folder)) target.key: target,
    }.values,
  ];

  @override
  void forget() => _targets = null;
}

/// [WatchScope.active]: one target, that of the document saved last; before any save, that of the
/// first document opened. When its last document closed, it is kept for [linger], then the target
/// of the document opened last (if one is open) takes its place.
final class TargetOfSavedDocument extends DocumentObserver implements WatchedTargets {
  TargetOfSavedDocument(this.linger);

  final Duration linger;

  /// The package of each open document, in the order they were opened.
  final _packages = <Uri, Directory?>{};

  /// What builds each package, by its path.
  final _targets = <String, BuildRunnerTarget?>{};

  /// The package whose target is watched.
  Directory? _active;

  /// Since when no open document is of the watched target.
  DateTime? _unopenedSince;

  @override
  void opened(Uri uri) {
    if (uri.scheme != 'file') {
      return;
    }
    final package = _packages[uri] = packageDirectory(File.fromUri(uri).parent);
    if (_targetOf(_active) == null) {
      _active = package;
    }
  }

  @override
  void saved(Uri uri) {
    if (uri.scheme != 'file') {
      return;
    }
    final package = _packages[uri] ?? packageDirectory(File.fromUri(uri).parent);
    if (_targetOf(package) != null) {
      _active = package;
    }
  }

  @override
  void closed(Uri uri) => _packages.remove(uri);

  @override
  List<BuildRunnerTarget> current(DateTime now) {
    final active = _targetOf(_active);
    if (active == null) {
      return const [];
    }
    final open = [
      for (final package in _packages.values)
        if (_targetOf(package) case final target?) target,
    ];
    if (open.any((target) => target.key == active.key)) {
      _unopenedSince = null;
      return [active];
    }
    if (now.difference(_unopenedSince ??= now) < linger) {
      return [active];
    }
    _unopenedSince = null;
    _active = open.isEmpty ? null : open.last.directory;
    return [if (open.isNotEmpty) open.last];
  }

  @override
  void forget() {
    _targets.clear();
    for (final uri in _packages.keys) {
      _packages[uri] = packageDirectory(File.fromUri(uri).parent);
    }
  }

  BuildRunnerTarget? _targetOf(Directory? package) => package == null
      ? null
      : _targets.putIfAbsent(normalizedPath(package), () => BuildRunnerTarget.of(package));
}

/// [WatchScope.open]: the targets that build a document open in the editor, and for [linger]
/// after the last one closed (a file closed and opened again keeps its watch).
final class TargetsOfOpenDocuments extends DocumentObserver implements WatchedTargets {
  TargetsOfOpenDocuments(this.linger);

  final Duration linger;

  /// The package of each open document.
  final _packages = <Uri, Directory?>{};

  /// What builds each package, by its path.
  final _targets = <String, BuildRunnerTarget?>{};

  /// The targets wanted, by key, and when they were last.
  final _wanted = <String, ({BuildRunnerTarget target, DateTime at})>{};

  @override
  void opened(Uri uri) {
    if (uri.scheme == 'file') {
      _packages[uri] = packageDirectory(File.fromUri(uri).parent);
    }
  }

  @override
  void closed(Uri uri) => _packages.remove(uri);

  @override
  List<BuildRunnerTarget> current(DateTime now) {
    final open = <String, BuildRunnerTarget>{
      for (final package in _packages.values.nonNulls)
        if (_targets.putIfAbsent(normalizedPath(package), () => BuildRunnerTarget.of(package))
            case final target?)
          target.key: target,
    };
    // A lingering target that builds a package an open one builds was replaced by it (its
    // configuration changed: a package joined a workspace, build_runner was upgraded).
    _wanted.removeWhere(
      (key, wanted) =>
          now.difference(wanted.at) > linger ||
          open.values.any(
            (target) =>
                target.key != key && target.packages.any((p) => wanted.target.builds(p.path)),
          ),
    );
    for (final target in open.values) {
      _wanted[target.key] = (target: target, at: now);
    }
    return [for (final wanted in _wanted.values) wanted.target];
  }

  @override
  void forget() {
    _targets.clear();
    for (final uri in _packages.keys) {
      _packages[uri] = packageDirectory(File.fromUri(uri).parent);
    }
  }
}

/// The watches of the [WatchedTargets], reporting to the editor.
final class BuildRunnerWatches {
  BuildRunnerWatches(this._targets, this._client, this._timings)
    : _summaries = BuildSummaries(_client, _timings.summary);

  final WatchedTargets _targets;
  final LspClient _client;
  final WatchTimings _timings;
  final BuildSummaries _summaries;
  final _watches = <String, _Watch>{};
  Timer? _timer;
  DateTime? _lastStart;
  DateTime? _lastRescan;
  var _stopped = false;

  /// Runs the watches the targets ask for, now and every [WatchTimings.poll].
  void start() {
    _sync(DateTime.now(), immediately: true);
    _timer = Timer.periodic(_timings.poll, (_) => _sync(DateTime.now()));
  }

  /// A pubspec.yaml or pubspec.lock changed: which targets there are is looked at again.
  void configurationChanged() => _targets.forget();

  /// Stops the watches, for good.
  void stop() {
    _stopped = true;
    _timer?.cancel();
    _summaries.cancel();
    for (final watch in _watches.values) {
      watch.kill();
    }
    _watches.clear();
  }

  void _sync(DateTime now, {bool immediately = false}) {
    if (_stopped) {
      return;
    }
    if (_lastRescan == null || now.difference(_lastRescan!) >= _timings.rescan) {
      _lastRescan = now;
      if (!immediately) {
        _targets.forget();
      }
    }
    _restartChanged(now, immediately: immediately);
    final wanted = {for (final target in _targets.current(now)) target.key: target};
    for (final key in [..._watches.keys]) {
      if (!wanted.containsKey(key)) {
        final watch = _watches.remove(key)!..kill();
        _log(watch.target, MessageType.log, 'stopped: no longer needed');
      }
    }
    _stopStuck(now);
    _startMissing(wanted.values, now);
  }

  /// Restarts the watches whose configuration changed, once two syncs in a row see the same change
  /// (a `pub get` writes several files), unless [immediately]. Changes right after a start are
  /// the start's own (see [WatchTimings.settle]).
  void _restartChanged(DateTime now, {required bool immediately}) {
    var restarted = false;
    for (final watch in [..._watches.values]) {
      final stamp = _stamp(watch.target);
      if (stamp == watch.stamp) {
        watch.pendingStamp = null;
      } else if (now.difference(watch.started) < _timings.settle) {
        watch.stamp = stamp;
      } else if (immediately || watch.pendingStamp == stamp) {
        _watches.remove(watch.target.key)!.kill();
        restarted = true;
      } else {
        watch.pendingStamp = stamp;
      }
    }
    if (restarted) {
      _targets.forget(); // what builds a package may have changed with its configuration
    }
  }

  /// Stops the watches that keep their lock longer than [WatchTimings.yieldLock] after another
  /// build_runner command asked for it; the next sync starts them again, and the new watch waits
  /// for the other command ("Waiting for already-running build_runner").
  void _stopStuck(DateTime now) {
    for (final watch in [..._watches.values]) {
      if (watch.process == null || !watch.target.lockRequests.any((file) => file.existsSync())) {
        watch.lockRequestedSince = null;
        continue;
      }
      final since = watch.lockRequestedSince ??= now;
      if (now.difference(since) >= _timings.yieldLock) {
        _watches.remove(watch.target.key)!.kill();
        _log(
          watch.target,
          MessageType.warning,
          'kept its lock ${_timings.yieldLock.inSeconds} s after another build_runner asked for '
          'it: restarting',
        );
      }
    }
  }

  /// Starts the watch of the first of [targets] that has none, unless one started less than
  /// [WatchTimings.stagger] ago (all of them without a stagger).
  void _startMissing(Iterable<BuildRunnerTarget> targets, DateTime now) {
    for (final target in targets) {
      if (_watches.containsKey(target.key)) {
        continue;
      }
      if (_lastStart case final last? when now.difference(last) < _timings.stagger) {
        return;
      }
      _lastStart = now;
      final watch = _Watch(target, _stamp(target), now);
      _watches[target.key] = watch;
      unawaited(_run(watch));
    }
  }

  Future<void> _run(_Watch watch) async {
    final target = watch.target;
    final Process process;
    try {
      // The Dart that runs this server: the one of the Flutter SDK, or fvm's.
      process = await Process.start(
        Platform.resolvedExecutable,
        target.arguments(BuildRunnerCommand.watch),
        workingDirectory: target.directory.path,
      );
    } on ProcessException catch (error) {
      _log(target, MessageType.error, 'could not start: ${error.message}');
      return;
    }
    watch.process = process;
    if (_stopped || _watches[target.key] != watch) {
      watch.kill(); // stopped, or replaced, while starting
      return;
    }
    // Whether build_runner gave its lock to another build_runner command (a `build` run by hand
    // or by a task): build_runner 2.17+ stops a watch for that.
    var yielded = false;
    void forward(Stream<List<int>> output, MessageType type) => output
        .transform(const Utf8Decoder(allowMalformed: true))
        .transform(const LineSplitter())
        .where((line) => line.trim().isNotEmpty)
        .listen((line) {
          yielded |= line.contains(_yieldedLock);
          _log(target, type, line);
          if (_buildSummary.firstMatch(line) case final summary?) {
            _summaries.report(target.name, line.trim(), failed: summary.group(1) == 'Failed');
          }
        });

    forward(process.stdout, MessageType.info);
    forward(process.stderr, MessageType.warning);
    final code = await process.exitCode;
    if (_stopped || _watches[target.key] != watch) {
      return; // stopped by this server
    }
    _log(target, code == 0 ? MessageType.info : MessageType.error, 'exited with code $code');
    // One that gave way to another build_runner, or ran for a while, starts again with the next
    // sync: the new one waits for the other command's lock. One that fails right after starting
    // stays stopped until its configuration changes, so a broken setup does not restart over and
    // over.
    if (yielded || DateTime.now().difference(watch.started) >= _timings.settle) {
      _watches.remove(target.key);
      _log(target, MessageType.info, 'restarting');
    }
  }

  void _log(BuildRunnerTarget target, MessageType type, String text) =>
      _client.log(type, 'build_runner ${target.name}: $text');

  /// When the configuration of [target] last changed, as text to compare.
  static String _stamp(BuildRunnerTarget target) => target.configuration.map(_modified).join(',');

  static int _modified(File file) {
    try {
      return file.lastModifiedSync().microsecondsSinceEpoch;
    } on FileSystemException {
      return 0;
    }
  }
}

/// A watch of [BuildRunnerWatches].
final class _Watch {
  _Watch(this.target, this.stamp, this.started);

  final BuildRunnerTarget target;
  final DateTime started;

  /// The configuration it was started with (or adopted while settling).
  String stamp;

  /// A changed configuration, seen once: restarted when the next sync sees the same.
  String? pendingStamp;

  /// Its process; null while it starts.
  Process? process;

  /// Since when another build_runner command has asked for its lock.
  DateTime? lockRequestedSince;

  void kill() {
    if (process case final process?) {
      killTree(process);
    }
  }
}

/// The Succeeded / Failed lines of the watches, shown to the user: those that come within
/// [window] of the first make one notification, so that a change that rebuilds several targets
/// shows once. Of one target, its last line counts.
final class BuildSummaries {
  BuildSummaries(this._client, this.window);

  final LspClient _client;
  final Duration window;
  final _latest = <String, ({bool failed, String line})>{};
  Timer? _timer;

  /// Reports the summary [line] of the build of [target].
  void report(String target, String line, {required bool failed}) {
    _latest[target] = (failed: failed, line: line);
    _timer ??= Timer(window, _show);
  }

  void cancel() => _timer?.cancel();

  void _show() {
    _timer = null;
    final reports = {..._latest};
    _latest.clear();
    final failed = [
      for (final entry in reports.entries)
        if (entry.value.failed) entry.key,
    ];
    final type = failed.isEmpty ? MessageType.info : MessageType.error;
    if (reports.length == 1) {
      final report = reports.entries.single;
      _client.show(type, 'build_runner ${report.key}: ${report.value.line}');
      return;
    }
    final succeeded = [
      for (final entry in reports.entries)
        if (!entry.value.failed) entry.key,
    ];
    _client.show(
      type,
      'build_runner: ${[if (failed.isNotEmpty) 'failed in ${failed.join(', ')}', if (succeeded.isNotEmpty) 'succeeded in ${succeeded.join(', ')}'].join('; ')}',
    );
  }
}

/// The build_runner watches as a feature of the language server: the scope from its settings,
/// the open documents from [documents], and changes of pubspec files from the editor, when it
/// reports them.
final class BuildRunnerFeature extends LspFeature {
  BuildRunnerFeature(this.documents, this._environment);

  final DocumentStore documents;
  final Map<String, String> _environment;
  WatchedTargets? _targets;
  BuildRunnerWatches? _watches;
  var _watchesFiles = false;

  @override
  Map<String, NotificationHandler> get notifications => {
    'workspace/didChangeWatchedFiles': (_) => _watches?.configurationChanged(),
  };

  @override
  void initialize(InitializeParams params) {
    _watchesFiles = params.watchesFiles;
    final linger = WatchTimings.from(_environment).linger;
    _targets = switch (WatchScope.from(params.initializationOptions, _environment)) {
      WatchScope.off => null,
      WatchScope.active => TargetOfSavedDocument(linger),
      WatchScope.open => TargetsOfOpenDocuments(linger),
      WatchScope.all => TargetsUnderFolders(params.workspaceFolders),
    };
    if (_targets case final DocumentObserver observer) {
      documents.observe(observer);
    }
  }

  @override
  void initialized(LspClient client) {
    final targets = _targets;
    if (targets == null) {
      return;
    }
    if (_watchesFiles) {
      client.request('client/registerCapability', {
        'registrations': [
          {
            'id': 'flutter-tools-pubspec',
            'method': 'workspace/didChangeWatchedFiles',
            'registerOptions': {
              'watchers': [
                {'globPattern': '**/pubspec.yaml'},
                {'globPattern': '**/pubspec.lock'},
              ],
            },
          },
        ],
      });
    }
    _watches = BuildRunnerWatches(targets, client, WatchTimings.from(_environment))..start();
  }

  @override
  void shutdown() => _watches?.stop();
}

final _buildSummary = RegExp(r'\b(Succeeded|Failed) after\b');

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
