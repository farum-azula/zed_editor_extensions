// A language server for Dart files, next to the Dart language server:
//
//     dart flutter_lsp.dart
//
// Its features, each in a file of its own:
//
// - snippet_completions.dart: the snippets of snippets.json as completions that add the imports
//   a snippet needs;
// - runner_installation.dart: copies the programs of the run, build and DevTools tasks
//   (languages/dart/tasks.json) to ~/.zed-flutter/, where the tasks find them;
// - build_runner_watches.dart: keeps `dart run build_runner watch` running for the packages that
//   generate code. By default for those with a file open in the editor; the `build_runner`
//   initialization option in the settings of flutter-tools (or FLUTTER_TOOLS_BUILD_RUNNER) says
//   `"all"` for every package of the workspace, or `"off"` (also `false`) for none.
//
// It writes nothing into projects itself (build_runner writes the generated files). A new feature
// is a subclass of LspFeature (lsp_server.dart) and one entry in the list below.

import 'dart:io';

import 'build_runner_watches.dart';
import 'lsp_server.dart';
import 'runner_installation.dart';
import 'snippet_completions.dart';

Future<void> main() async {
  final here = File.fromUri(Platform.script).parent;
  final documents = DocumentStore();
  final server = LspServer('flutter-tools', [
    documents,
    SnippetCompletions(loadSnippets(File('${here.path}/snippets.json')), documents),
    RunnerInstallation(here),
    BuildRunnerFeature(documents, Platform.environment),
  ]);
  exit(await server.serve());
}
