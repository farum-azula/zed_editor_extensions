// A language server for Dart files, next to the Dart language server, with two small jobs:
//
//     dart flutter_lsp.dart
//
// - The snippets (snippets.json) come from here as completions, which, unlike Zed's snippets, can
//   carry edits that add the imports a snippet needs (`log` dart:developer, `scf`
//   flutter/material.dart, ...) when the file does not have them yet.
// - The `Flutter: run on <platform>` tasks of the Dart language (languages/dart/tasks.json) start
//   `~/.zed-flutter/flutter_run.dart`: a task cannot name the extension's own directory, which
//   differs from one OS to the next. On start, this server copies flutter_run.dart and devices.dart
//   from next to itself to ~/.zed-flutter/, when they differ.
//
// It writes nothing into projects.

import 'dart:convert';
import 'dart:io';

import 'protocol.dart';

/// Where the tasks find the runner: the same path on every machine.
const runnerDir = '.zed-flutter';

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
  await for (final message in stdin.transform(DapDecoder())) {
    final id = message['id'];
    final params =
        (message['params'] as Map?)?.cast<String, Object?>() ?? const {};
    switch (message['method']) {
      case 'initialize':
        final log = <String>[];
        try {
          installRunner(here, log.add);
        } on Object catch (error) {
          log.add(
            'Flutter Tools could not install the runner of the run tasks: $error',
          );
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
        _send({
          'id': id,
          'result': {
            'isIncomplete': false,
            'items': completions(snippets, documents[uri] ?? ''),
          },
        });
      case 'shutdown':
        _send({'id': id, 'result': null});
      case 'exit':
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

/// The snippets as completion items for a file with [text]: each adds the imports it needs that
/// the file has none that brings.
List<Map<String, Object?>> completions(List<Snippet> snippets, String text) => [
  for (final snippet in snippets)
    {
      'label': snippet.prefix,
      'filterText': snippet.prefix,
      'kind': 15, // Snippet
      'detail': snippet.name,
      'documentation': '${snippet.description}\n\n`${snippet.body}`',
      'insertTextFormat': 2, // Snippet
      'insertText': snippet.body,
      if (importEdits(text, snippet.imports) case final edits
          when edits.isNotEmpty)
        'additionalTextEdits': edits,
    },
];

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

/// Copies the runner next to this file to ~/.zed-flutter/, when it differs.
void installRunner(Directory source, void Function(String) log) {
  final home =
      Platform.environment['HOME'] ?? Platform.environment['USERPROFILE'];
  if (home == null) {
    log(
      'Flutter Tools: no home directory, so the run tasks will not find their runner',
    );
    return;
  }
  final target = Directory('$home/$runnerDir')..createSync(recursive: true);
  for (final name in ['flutter_run.dart', 'devices.dart']) {
    final text = File('${source.path}/$name').readAsStringSync();
    final to = File('${target.path}/$name');
    if (!to.existsSync() || to.readAsStringSync() != text) {
      to.writeAsStringSync(text);
    }
  }
}
