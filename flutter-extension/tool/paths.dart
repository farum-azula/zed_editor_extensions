// Paths the tools compare and the home directory they write into, the same on every OS.

import 'dart:io';

/// The user's home directory from [environment] (`HOME`, or `USERPROFILE` on Windows); null when
/// it names none.
String? homeDirectory([Map<String, String>? environment]) {
  final variables = environment ?? Platform.environment;
  return variables['HOME'] ?? variables['USERPROFILE'];
}

/// [directory] as an absolute path without `.`, `..` or a trailing separator, to compare.
String normalizedPath(Directory directory) =>
    directory.absolute.uri.normalizePath().toFilePath().replaceFirst(RegExp(r'[\\/]$'), '');

/// Whether the file or directory at [path] is [directory] or inside it.
bool isWithin(Directory directory, String path) {
  final root = normalizedPath(directory);
  final normalized = normalizedPath(Directory(path));
  return normalized == root || normalized.startsWith('$root${Platform.pathSeparator}');
}

/// The last segment of [path].
String basename(String path) => path.split(RegExp(r'[\\/]')).where((s) => s.isNotEmpty).last;

/// [path] with `/` separators, no trailing one, and in lower case where the file system ignores
/// case (macOS, Windows): two spellings of one directory give the same text.
String comparablePath(String path) {
  var slashed = path.replaceAll('\\', '/');
  while (slashed.length > 1 && slashed.endsWith('/')) {
    slashed = slashed.substring(0, slashed.length - 1);
  }
  return Platform.isWindows || Platform.isMacOS ? slashed.toLowerCase() : slashed;
}
