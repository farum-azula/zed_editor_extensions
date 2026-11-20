// Where build_runner runs for a package, and with which arguments. Shared by the language
// server's watches (build_runner_watches.dart), the `dart run` task (dart_run.dart) and the
// build_runner tasks (build_runner_run.dart), so that all of them use one build graph per package:
// two graphs over the same outputs (a package's own and its workspace's) delete each other's files.

import 'dart:io';

import 'paths.dart';
import 'pubspec.dart';

/// The commands of build_runner the tools run.
enum BuildRunnerCommand { build, watch }

/// The first build_runner that builds a whole pub workspace with `--workspace`.
const workspaceBuildsSince = [2, 14];

/// How deep below a folder of the editor packages are looked for (`example/flutter_app` is 2).
const maxPackageDepth = 4;

/// Directories never looked into for packages: build output, other tools' dependencies.
const _skippedDirectories = {'build', 'node_modules', 'Pods', 'target'};

/// One build_runner: started in [directory], building [packages]. A pub workspace whose
/// build_runner builds workspaces has one for all its packages ([workspace] set): run with
/// `--workspace` in its root when the root uses build_runner, else in the first member that does.
/// Any other package that uses build_runner has its own.
final class BuildRunnerTarget {
  BuildRunnerTarget._(this.directory, this.packages, {this.workspace});

  /// The target that builds [package]: its workspace's (see the class), or its own when it uses
  /// build_runner; null when nothing builds it.
  static BuildRunnerTarget? of(Directory package) {
    if (_workspaceRootOf(package) case final root? when _buildsWorkspaces(root)) {
      return _ofWorkspace(root);
    }
    return usesBuildRunner(package)
        ? BuildRunnerTarget._(package.absolute, [package.absolute])
        : null;
  }

  /// The targets of the packages at and below [root], down to [maxPackageDepth] levels (hidden
  /// directories and build output left out), each once, in the order of the packages.
  static List<BuildRunnerTarget> under(Directory root) {
    final targets = <String, BuildRunnerTarget>{};
    for (final package in _packagesUsingBuildRunner(root)) {
      if (of(package) case final target?) {
        targets.putIfAbsent(target.key, () => target);
      }
    }
    return [...targets.values];
  }

  /// Where `dart run build_runner` runs.
  final Directory directory;

  /// What it builds: the package, or every package of the workspace.
  final List<Directory> packages;

  /// The root of the pub workspace it builds; null when it builds one package.
  final Directory? workspace;

  /// Its name in messages: the name of its directory.
  String get name => basename(directory.path);

  /// What tells it from another target: one build graph per key.
  String get key => '${normalizedPath(directory)}${workspace == null ? '' : ' --workspace'}';

  /// The `dart` arguments that run build_runner's [command] for this target.
  List<String> arguments(BuildRunnerCommand command) => [
    'run',
    'build_runner',
    command.name,
    '--delete-conflicting-outputs',
    if (workspace != null) '--workspace',
  ];

  /// Whether it builds the file or directory at [path].
  bool builds(String path) => packages.any((package) => isWithin(package, path));

  /// The files build_runner reads once, at its start: a change to one (a dependency added, a
  /// `pub get`, a member added to the workspace) needs a new build_runner. A member of a workspace
  /// has no pubspec.lock nor package_config.json of its own: the root's change instead.
  List<File> get configuration => [
    for (final root in {
      normalizedPath(directory),
      if (workspace case final root?) normalizedPath(root),
    })
      for (final name in const ['pubspec.yaml', 'pubspec.lock', '.dart_tool/package_config.json'])
        File('$root/$name'),
  ];

  /// The files another build_runner command (a `build` by hand) writes to ask this one for its
  /// lock.
  List<File> get lockRequests => [
    File('${directory.path}/.dart_tool/build/lock/build_runner.lock.requested'),
    if (workspace case final root?)
      File('${root.path}/.dart_tool/build/lock/build_runner.workspace.lock.requested'),
  ];

  static BuildRunnerTarget? _ofWorkspace(Directory root) {
    final members = workspaceMembers(root).map((member) => member.absolute).toList()
      ..sort(_byPathSegments);
    final packages = [root.absolute, ...members];
    for (final package in packages) {
      if (usesBuildRunner(package)) {
        return BuildRunnerTarget._(package, packages, workspace: root.absolute);
      }
    }
    return null;
  }

  /// The root of the pub workspace [package] belongs to: itself, or the ancestor that lists it
  /// under `workspace:` (any ancestor: `workspace: [example/app]` lists a member two levels down).
  static Directory? _workspaceRootOf(Directory package) {
    final path = normalizedPath(package);
    for (var candidate = package.absolute; ; candidate = candidate.parent) {
      final members = workspaceMembers(candidate);
      if (members.isNotEmpty &&
          (normalizedPath(candidate) == path ||
              members.any((member) => normalizedPath(member) == path))) {
        return candidate;
      }
      if (candidate.parent.path == candidate.path) {
        return null;
      }
    }
  }

  static bool _buildsWorkspaces(Directory root) => switch (lockedVersion(root, 'build_runner')) {
    final version? => versionAtLeast(version, workspaceBuildsSince),
    null => false,
  };

  static int _byPathSegments(Directory a, Directory b) {
    final left = normalizedPath(a).split(RegExp(r'[\\/]'));
    final right = normalizedPath(b).split(RegExp(r'[\\/]'));
    for (var i = 0; i < left.length && i < right.length; i++) {
      if (left[i].compareTo(right[i]) case final order when order != 0) {
        return order;
      }
    }
    return left.length.compareTo(right.length);
  }

  static List<Directory> _packagesUsingBuildRunner(Directory root) {
    final packages = <Directory>[];
    void visit(Directory directory, int depth) {
      if (usesBuildRunner(directory)) {
        packages.add(directory);
      }
      if (depth == maxPackageDepth) {
        return;
      }
      final List<Directory> children;
      try {
        children = directory.listSync(followLinks: false).whereType<Directory>().where((child) {
          final name = basename(child.path);
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
}
