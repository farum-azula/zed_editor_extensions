// The programs of the run, build and DevTools tasks of the Dart language (languages/dart/tasks.json)
// run from ~/.zed-flutter/: a task cannot name the extension's own directory, which differs from
// one OS to the next. The language server copies them there on start.

import 'dart:io';

import 'lsp_server.dart';
import 'paths.dart';

/// Where the tasks find their programs: the same path on every machine.
const runnerDir = '.zed-flutter';

/// Copies the programs of the tasks, the `.dart` files next to the server ([source]), to
/// ~/[runnerDir]/ when the server starts.
final class RunnerInstallation extends LspFeature {
  RunnerInstallation(this.source);

  final Directory source;
  final _problems = <String>[];

  @override
  void initialize(InitializeParams params) {
    try {
      if (!installRunner(source, Platform.environment)) {
        _problems.add('Flutter Tools: no home directory, so the tasks will not find their runner');
      }
    } on Object catch (error) {
      _problems.add('Flutter Tools could not install the runner of the tasks: $error');
    }
  }

  @override
  void initialized(LspClient client) {
    for (final problem in _problems) {
      client.log(MessageType.error, problem);
    }
  }
}

/// Copies the `.dart` files of [source] that differ to [runnerDir] in the home directory of
/// [environment]; false when it names none.
bool installRunner(Directory source, Map<String, String> environment) {
  final home = homeDirectory(environment);
  if (home == null) {
    return false;
  }
  final target = Directory('$home/$runnerDir')..createSync(recursive: true);
  for (final file in source.listSync().whereType<File>()) {
    final name = file.uri.pathSegments.last;
    if (!name.endsWith('.dart')) {
      continue;
    }
    final text = file.readAsStringSync();
    final copy = File('${target.path}/$name');
    if (!copy.existsSync() || copy.readAsStringSync() != text) {
      copy.writeAsStringSync(text);
    }
  }
  return true;
}
