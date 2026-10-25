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
  exit(await dart(['run', file.path, ...args.skip(1)], directory));
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
