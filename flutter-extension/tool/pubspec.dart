// What the tools read from a package's pubspec.yaml and pubspec.lock. Read by lines: each tool
// runs as plain `dart <file>`, without a `pub get`, so none of them brings a YAML parser.

import 'dart:convert';
import 'dart:io';

import 'paths.dart';

/// The package [start] is in: the nearest directory at or above it with a pubspec.yaml; null when
/// there is none up to the file system's root.
Directory? packageDirectory(Directory start) {
  for (var directory = start.absolute; ; directory = directory.parent) {
    if (File('${directory.path}${Platform.pathSeparator}pubspec.yaml').existsSync()) {
      return directory;
    }
    if (directory.parent.path == directory.path) {
      return null;
    }
  }
}

/// The text of [package]'s pubspec.yaml; null when it has none or it cannot be read.
String? readPubspec(Directory package) {
  try {
    return File('${package.path}${Platform.pathSeparator}pubspec.yaml').readAsStringSync();
  } on FileSystemException {
    return null;
  }
}

/// Whether [package]'s pubspec.yaml lists `build_runner` among its `dev_dependencies`.
bool usesBuildRunner(Directory package) {
  final pubspec = readPubspec(package);
  return pubspec != null && devDependsOnBuildRunner(pubspec);
}

/// Whether [package]'s pubspec.yaml depends on the Flutter SDK.
bool isFlutterPackage(Directory package) {
  final pubspec = readPubspec(package);
  return pubspec != null && dependsOnFlutter(pubspec);
}

final _buildRunnerKey = RegExp(r"""^['"]?build_runner['"]?\s*:""");
final _flutterKey = RegExp(r"""^['"]?flutter['"]?\s*:""");
final _sdkFlutter = RegExp(r"""^sdk\s*:\s*['"]?flutter['"]?\s*(#.*)?$""");

/// Whether [pubspec] lists `build_runner` among its `dev_dependencies`: the key at the
/// indentation of the section's first entry (not a key of a dependency's own settings).
bool devDependsOnBuildRunner(String pubspec) {
  int? indent;
  for (final line in _section(pubspec, 'dev_dependencies')) {
    indent ??= line.depth;
    if (line.depth == indent && _buildRunnerKey.hasMatch(line.entry)) {
      return true;
    }
  }
  return false;
}

/// Whether [pubspec] has `flutter: sdk: flutter` among its `dependencies`.
bool dependsOnFlutter(String pubspec) {
  int? indent;
  var inFlutter = false;
  for (final line in _section(pubspec, 'dependencies')) {
    indent ??= line.depth;
    if (line.depth == indent) {
      inFlutter = _flutterKey.hasMatch(line.entry);
    } else if (inFlutter && _sdkFlutter.hasMatch(line.entry)) {
      return true;
    }
  }
  return false;
}

/// The members of the pub workspace whose root is [root]: the paths its pubspec.yaml lists under
/// `workspace:`, a glob (`packages/*`) standing for each package it matches; none when it lists
/// none (or has no pubspec.yaml).
List<Directory> workspaceMembers(Directory root) => [
  for (final line in _section(readPubspec(root) ?? '', 'workspace'))
    if (line.entry.startsWith('-'))
      if (_unquoted(line.entry.substring(1).split('#').first.trim()) case final path
          when path.isNotEmpty)
        ...(_isGlob(path)
            ? _packagesMatching(root, path)
            : [Directory('${root.path}${Platform.pathSeparator}$path')]),
];

bool _isGlob(String path) => path.contains(RegExp(r'[*?\[{]'));

/// The packages under [root] whose path relative to it matches [glob]: `*` and `?` within a
/// segment, `**` for any number of segments, `[abc]` and `{a,b}`, as pub reads them.
List<Directory> _packagesMatching(Directory root, String glob) {
  final pattern = RegExp('^${_globToRegExp(glob.replaceFirst(RegExp(r'/+$'), ''))}\$');
  final found = <Directory>[];
  void visit(Directory directory, String relative, int depth) {
    final List<FileSystemEntity> children;
    try {
      children = directory.listSync(followLinks: false);
    } on FileSystemException {
      return;
    }
    for (final child in children.whereType<Directory>()) {
      final name = basename(child.path);
      if (name.startsWith('.') || name == 'build') {
        continue;
      }
      final path = relative.isEmpty ? name : '$relative/$name';
      if (pattern.hasMatch(path) &&
          File('${child.path}${Platform.pathSeparator}pubspec.yaml').existsSync()) {
        found.add(child);
      }
      if (depth < _globDepth) {
        visit(child, path, depth + 1);
      }
    }
  }

  visit(root, '', 1);
  return found..sort((a, b) => a.path.compareTo(b.path));
}

/// How deep below the root a glob's members are looked for.
const _globDepth = 6;

String _globToRegExp(String glob) {
  final out = StringBuffer();
  for (var i = 0; i < glob.length; i++) {
    final char = glob[i];
    if (glob.startsWith('**/', i)) {
      out.write('(?:[^/]+/)*');
      i += 2;
    } else if (glob.startsWith('**', i)) {
      out.write('.*');
      i += 1;
    } else if (char == '*') {
      out.write('[^/]*');
    } else if (char == '?') {
      out.write('[^/]');
    } else if (char == '[') {
      final close = glob.indexOf(']', i + 1);
      if (close < 0) {
        out.write(RegExp.escape(char));
      } else {
        final body = glob.substring(i + 1, close);
        out.write('[${body.startsWith('!') ? '^${body.substring(1)}' : body}]');
        i = close;
      }
    } else if (char == '{') {
      final close = glob.indexOf('}', i + 1);
      if (close < 0) {
        out.write(RegExp.escape(char));
      } else {
        out.write('(?:${glob.substring(i + 1, close).split(',').map(_globToRegExp).join('|')})');
        i = close;
      }
    } else {
      out.write(RegExp.escape(char));
    }
  }
  return out.toString();
}

/// The version of [package] in [root]'s pubspec.lock, e.g. `2.17.1`; null without one.
String? lockedVersion(Directory root, String package) {
  final String text;
  try {
    text = File('${root.path}${Platform.pathSeparator}pubspec.lock').readAsStringSync();
  } on FileSystemException {
    return null;
  }
  var inPackage = false;
  for (final line in LineSplitter.split(text)) {
    if (RegExp(r'^  \S').hasMatch(line)) {
      inPackage = line.trim() == '$package:';
    } else if (inPackage) {
      if (RegExp(r'''^    version:\s*['"]?([^'"\s]+)''').firstMatch(line) case final version?) {
        return version.group(1);
      }
    }
  }
  return null;
}

/// Whether [version] (`2.17.1`, `2.14.0-dev`) is [minimum] (`[2, 14]`) or later.
bool versionAtLeast(String version, List<int> minimum) {
  final parts = version.split(RegExp(r'[.+-]')).map(int.tryParse).toList();
  for (var i = 0; i < minimum.length; i++) {
    final part = i < parts.length ? parts[i] ?? 0 : 0;
    if (part != minimum[i]) {
      return part > minimum[i];
    }
  }
  return true;
}

/// The lines of the top-level [name] section of [pubspec], comments and blank lines left out,
/// each with its indentation.
Iterable<({int depth, String entry})> _section(String pubspec, String name) sync* {
  final header = RegExp('^$name\\s*:');
  var inSection = false;
  for (final line in LineSplitter.split(pubspec)) {
    final entry = line.trimLeft();
    if (entry.isEmpty || entry.startsWith('#')) {
      continue;
    }
    final depth = line.length - entry.length;
    if (depth == 0) {
      inSection = header.hasMatch(line);
    } else if (inSection) {
      yield (depth: depth, entry: entry.trimRight());
    }
  }
}

String _unquoted(String text) => text.replaceAll(RegExp('''^['"]|['"]\$'''), '');
