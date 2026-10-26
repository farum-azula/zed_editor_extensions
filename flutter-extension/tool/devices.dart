// Turns a run target (`ios-simulator`, `android`, `web`, `macos`, `windows`, `linux`) into the id of
// a device `flutter run -d` can use, starting a simulator or an emulator when none is running.

import 'dart:async';
import 'dart:convert';
import 'dart:io';

/// The run targets, in the order they are offered, with their names.
const targets = {
  'ios-simulator': 'iOS Simulator',
  'android': 'Android',
  'web': 'Web',
  'macos': 'macOS',
  'windows': 'Windows',
  'linux': 'Linux',
};

class DeviceException implements Exception {
  DeviceException(this.message);

  final String message;

  @override
  String toString() => message;
}

/// How often the device list is read again while a simulator or an emulator boots.
Duration get pollInterval => Duration(
  milliseconds: int.tryParse(Platform.environment['FLUTTER_TOOLS_POLL_MS'] ?? '') ?? 2000,
);

/// How long a simulator or an emulator may take to boot.
const bootTimeout = Duration(minutes: 5);

/// The device id for [target]. [flutter] is the command that runs Flutter (`flutter`, or `fvm
/// flutter`); [log] reports what is happening, a line at a time.
Future<String> resolveDevice(String target, List<String> flutter, void Function(String) log) async {
  switch (target) {
    case 'macos' || 'windows' || 'linux':
      final host = Platform.operatingSystem;
      if (host != target) {
        throw DeviceException('${targets[target]} apps run only on $target; this is $host.');
      }
      return target;
    case 'web':
      final devices = await listDevices(flutter);
      for (final id in ['chrome', 'edge']) {
        if (devices.any((d) => d.id == id)) {
          return id;
        }
      }
      log('No Chrome or Edge found: serving the app on web-server; open the URL it prints.');
      return 'web-server';
    case 'ios-simulator':
      final simulator = await _deviceOrBoot(
        flutter,
        log,
        (d) => d.platform == 'ios' && d.emulator,
        'an iOS Simulator',
        () => _bootSimulator(log),
      );
      // A simulator can be booted with no window, Simulator.app closed: the app would run where
      // nobody sees it. Opening Simulator.app on it shows its window, and is harmless when it is
      // showing already.
      await _showSimulator(simulator);
      return simulator;
    case 'android':
      return _deviceOrBoot(
        flutter,
        log,
        (d) => d.platform.startsWith('android'),
        'an Android device or emulator',
        () => _launchEmulator(flutter, log),
      );
    default:
      throw DeviceException('unknown target "$target"; use one of ${targets.keys.join(', ')}');
  }
}

class Device {
  Device(this.id, this.name, this.platform, this.emulator);

  final String id;
  final String name;
  final String platform;
  final bool emulator;
}

Future<List<Device>> listDevices(List<String> flutter) async {
  final result = await runFlutter(flutter, ['devices', '--machine']);
  if (result.exitCode != 0) {
    throw DeviceException('`flutter devices` failed: ${result.stderr}');
  }
  final out = result.stdout as String;
  final start = out.indexOf('[');
  if (start < 0) {
    return [];
  }
  final list = jsonDecode(out.substring(start)) as List;
  return [
    for (final d in list.cast<Map<String, Object?>>())
      Device(
        d['id'] as String,
        d['name'] as String? ?? '',
        d['targetPlatform'] as String? ?? '',
        d['emulator'] == true,
      ),
  ];
}

Future<ProcessResult> runFlutter(List<String> flutter, List<String> args) =>
    Process.run(flutter.first, [...flutter.skip(1), ...args], runInShell: Platform.isWindows);

Future<String> _deviceOrBoot(
  List<String> flutter,
  void Function(String) log,
  bool Function(Device) wanted,
  String what,
  Future<void> Function() boot,
) async {
  final running = (await listDevices(flutter)).where(wanted);
  if (running.isNotEmpty) {
    return running.first.id;
  }
  await boot();
  final deadline = DateTime.now().add(bootTimeout);
  while (DateTime.now().isBefore(deadline)) {
    await Future<void>.delayed(pollInterval);
    final booted = (await listDevices(flutter)).where(wanted);
    if (booted.isNotEmpty) {
      log('${booted.first.name} is ready.');
      return booted.first.id;
    }
  }
  throw DeviceException('$what did not come up within ${bootTimeout.inMinutes} minutes.');
}

/// Boots an iPhone simulator of the newest iOS runtime and opens the Simulator app.
Future<void> _bootSimulator(void Function(String) log) async {
  if (!Platform.isMacOS) {
    throw DeviceException('the iOS Simulator runs only on macOS.');
  }
  final list = await Process.run('xcrun', ['simctl', 'list', 'devices', 'available', '-j']);
  if (list.exitCode != 0) {
    throw DeviceException('`xcrun simctl` failed; is Xcode installed? ${list.stderr}');
  }
  final runtimes = (jsonDecode(list.stdout as String)['devices'] as Map).cast<String, List>();
  final ios = runtimes.keys.where((r) => r.contains('.iOS-')).toList()..sort(_compareRuntimes);
  for (final runtime in ios.reversed) {
    final phones = runtimes[runtime]!.cast<Map>().where((d) => '${d['name']}'.startsWith('iPhone'));
    if (phones.isEmpty) {
      continue;
    }
    final phone = phones.first;
    log('Starting the iOS Simulator (${phone['name']})…');
    await Process.run('xcrun', ['simctl', 'boot', '${phone['udid']}']);
    return;
  }
  throw DeviceException(
    'no iPhone simulator is installed; add one in Xcode → Settings → Components.',
  );
}

/// Opens Simulator.app on the simulator [udid]. `FLUTTER_TOOLS_OPEN_SIMULATOR` (a JSON list) is the
/// command to run instead, given the udid, for the tests.
Future<void> _showSimulator(String udid) async {
  final override = Platform.environment['FLUTTER_TOOLS_OPEN_SIMULATOR'];
  final command = override != null
      ? [...(jsonDecode(override) as List).cast<String>(), udid]
      : ['open', '-a', 'Simulator', '--args', '-CurrentDeviceUDID', udid];
  await Process.run(command.first, command.skip(1).toList());
}

/// `...SimRuntime.iOS-18-2` before `...SimRuntime.iOS-26-0`.
int _compareRuntimes(String a, String b) {
  List<int> version(String r) =>
      r.split('.iOS-').last.split('-').map((p) => int.tryParse(p) ?? 0).toList();
  final va = version(a), vb = version(b);
  for (var i = 0; i < va.length && i < vb.length; i++) {
    if (va[i] != vb[i]) {
      return va[i].compareTo(vb[i]);
    }
  }
  return va.length.compareTo(vb.length);
}

/// Launches the first Android emulator `flutter emulators` lists.
Future<void> _launchEmulator(List<String> flutter, void Function(String) log) async {
  final result = await runFlutter(flutter, ['emulators']);
  final emulators = [
    for (final line in LineSplitter.split(result.stdout as String))
      if (line.contains('•') && line.trim().endsWith('android'))
        line.split('•').map((p) => p.trim()).toList(),
  ];
  if (emulators.isEmpty) {
    throw DeviceException(
      'no Android device is connected and no emulator is set up; '
      'create one with `flutter emulators --create` or in Android Studio.',
    );
  }
  log('Starting the Android emulator ${emulators.first[1]}…');
  final launch = await runFlutter(flutter, ['emulators', '--launch', emulators.first[0]]);
  if (launch.exitCode != 0) {
    throw DeviceException('`flutter emulators --launch` failed: ${launch.stdout}${launch.stderr}');
  }
}

/// The command that runs Flutter for the project in [dir]: `fvm flutter` when the project pins a
/// Flutter version with fvm, `flutter` otherwise. `FLUTTER_TOOLS_FLUTTER` (a JSON list) overrides it.
List<String> flutterCommand(Directory dir) {
  final override = Platform.environment['FLUTTER_TOOLS_FLUTTER'];
  if (override != null) {
    return (jsonDecode(override) as List).cast<String>();
  }
  final fvm = File('${dir.path}/.fvmrc').existsSync() || Directory('${dir.path}/.fvm').existsSync();
  return fvm ? ['fvm', 'flutter'] : ['flutter'];
}

/// The directory of the Flutter app to run from [start] (the worktree root, as tasks and
/// scenarios give it): [start] itself when it is one; otherwise, in a monorepo or a pub
/// workspace, the app under it, i.e. a package that depends on the Flutter SDK and has a
/// `lib/main.dart`. The members of [start]'s `workspace:` are looked at first, then its
/// subdirectories, a few levels down. Of several apps, the one holding [activeFile] (Zed's
/// `ZED_FILE` by default) is taken; without one, a [DeviceException] lists them. With none,
/// [start] is returned, and Flutter reports what it misses there.
Directory flutterApp(Directory start, {String? activeFile}) {
  if (isFlutterApp(start)) {
    return start;
  }
  var apps = [
    for (final member in workspaceMembers(start))
      if (isFlutterApp(member)) member,
  ];
  if (apps.isEmpty) {
    apps = _appsBelow(start, depth: 3);
  }
  if (apps.isEmpty) {
    return start;
  }
  if (apps.length == 1) {
    return apps.single;
  }
  final file = activeFile ?? Platform.environment['ZED_FILE'];
  if (file != null && file.isNotEmpty) {
    final path = _canonical(file);
    for (final app in apps) {
      if (path.startsWith('${_canonical(app.path)}${Platform.pathSeparator}')) {
        return app;
      }
    }
  }
  throw DeviceException(
    'this folder holds several Flutter apps (${apps.map((a) => _relative(a, start)).join(', ')}); '
    'open a file of the one to run, or set "cwd" in a scenario of your own.',
  );
}

/// [path] to compare with another: absolute, links resolved (on Windows also the short 8.3 names
/// of folders, `RUNNER~1`, which a path may hold or not), and lower case on Windows, whose paths
/// ignore case.
String _canonical(String path) {
  var canonical = File(path).absolute.path;
  try {
    canonical = File(path).resolveSymbolicLinksSync();
  } on FileSystemException {
    // Not there (any more): as given.
  }
  return Platform.isWindows ? canonical.toLowerCase() : canonical;
}

/// Whether [dir] is a Flutter app: its pubspec.yaml depends on the Flutter SDK and it has a
/// `lib/main.dart`.
bool isFlutterApp(Directory dir) {
  final sep = Platform.pathSeparator;
  final pubspec = File('${dir.path}${sep}pubspec.yaml');
  return pubspec.existsSync() &&
      File('${dir.path}${sep}lib${sep}main.dart').existsSync() &&
      pubspecDependsOnFlutter(pubspec.readAsStringSync());
}

/// The directories of the `workspace:` list in [root]'s pubspec.yaml (pub workspaces).
List<Directory> workspaceMembers(Directory root) {
  final pubspec = File('${root.path}${Platform.pathSeparator}pubspec.yaml');
  if (!pubspec.existsSync()) {
    return const [];
  }
  final members = <Directory>[];
  var inWorkspace = false;
  for (final line in LineSplitter.split(pubspec.readAsStringSync())) {
    final entry = line.trim();
    if (entry.isEmpty || entry.startsWith('#')) {
      continue;
    }
    if (line.length == line.trimLeft().length) {
      inWorkspace = RegExp(r'^workspace\s*:').hasMatch(line);
    } else if (inWorkspace && entry.startsWith('-')) {
      final path = entry
          .substring(1)
          .split('#')
          .first
          .trim()
          .replaceAll(RegExp('''^['"]|['"]\$'''), '');
      if (path.isNotEmpty) {
        members.add(Directory('${root.path}${Platform.pathSeparator}$path'));
      }
    }
  }
  return members;
}

/// Whether [pubspec] has `flutter: sdk: flutter` among its `dependencies` (read by lines: the
/// tools bring no YAML parser).
bool pubspecDependsOnFlutter(String pubspec) {
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
        inFlutter = RegExp(r'''^['"]?flutter['"]?\s*:''').hasMatch(entry);
      } else if (inFlutter &&
          RegExp(r'''^sdk\s*:\s*['"]?flutter['"]?\s*(#.*)?$''').hasMatch(entry)) {
        return true;
      }
    }
  }
  return false;
}

/// Directories skipped while looking for apps: tool output, dependencies and platform projects.
const _skipped = {
  'build',
  'node_modules',
  'android',
  'ios',
  'macos',
  'linux',
  'windows',
  'web',
  'Pods',
};

List<Directory> _appsBelow(Directory dir, {required int depth}) {
  if (depth == 0) {
    return const [];
  }
  final apps = <Directory>[];
  final List<FileSystemEntity> children;
  try {
    children = dir.listSync(followLinks: false)..sort((a, b) => a.path.compareTo(b.path));
  } on FileSystemException {
    return const [];
  }
  for (final child in children.whereType<Directory>()) {
    final name = child.uri.pathSegments.where((s) => s.isNotEmpty).last;
    if (name.startsWith('.') || _skipped.contains(name)) {
      continue;
    }
    if (isFlutterApp(child)) {
      apps.add(child);
    } else {
      apps.addAll(_appsBelow(child, depth: depth - 1));
    }
  }
  return apps;
}

String _relative(Directory dir, Directory root) {
  final prefix = '${root.absolute.path}${Platform.pathSeparator}';
  final path = dir.absolute.path;
  return path.startsWith(prefix) ? path.substring(prefix.length) : path;
}

/// Changes under [root] and every directory below it, on every platform. Dart watches
/// subdirectories itself on macOS and Windows; on Linux (inotify) it does not, so there each
/// directory gets its own watch, and directories created later are added as they appear.
/// Directories whose name [skip] accepts (default: hidden ones and `build`) are not watched on
/// Linux; elsewhere their events still come, so callers filter paths as well.
/// [perDirectory] forces either way (tests run the Linux way on any OS).
Stream<FileSystemEvent> watchTree(
  Directory root, {
  bool Function(String name)? skip,
  bool? perDirectory,
}) {
  if (!(perDirectory ?? Platform.isLinux)) {
    return root.watch(recursive: true);
  }
  final skipped = skip ?? (name) => name.startsWith('.') || name == 'build';
  final watches = <String, StreamSubscription<FileSystemEvent>>{};
  late final StreamController<FileSystemEvent> controller;

  void watchTreeOf(Directory directory) {
    if (watches.containsKey(directory.path)) {
      return;
    }
    try {
      watches[directory.path] = directory.watch().listen((event) {
        controller.add(event);
        if (event is FileSystemCreateEvent && event.isDirectory) {
          watchTreeOf(Directory(event.path));
        } else if (event is FileSystemDeleteEvent || event is FileSystemMoveEvent) {
          watches.remove(event.path)?.cancel();
        }
      }, onError: controller.addError);
    } on FileSystemException catch (error) {
      controller.addError(error);
      return;
    }
    final List<FileSystemEntity> children;
    try {
      children = directory.listSync(followLinks: false);
    } on FileSystemException {
      return; // gone again, or not readable
    }
    for (final child in children) {
      if (child is Directory) {
        final name = child.uri.pathSegments.where((s) => s.isNotEmpty).last;
        if (!skipped(name)) {
          watchTreeOf(child);
        }
      }
    }
  }

  controller = StreamController<FileSystemEvent>.broadcast(
    onListen: () => watchTreeOf(root),
    onCancel: () {
      for (final watch in watches.values) {
        watch.cancel();
      }
      watches.clear();
    },
  );
  return controller.stream;
}
