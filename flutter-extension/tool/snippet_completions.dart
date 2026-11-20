// The snippets of snippets.json as completions of the language server: unlike Zed's own snippets,
// a completion can carry edits that add the imports a snippet needs (`log` dart:developer, `scf`
// flutter/material.dart, ...) when the file does not have them yet. They are offered for the word
// being typed (`sc` offers `sc`, `scf`, ...), never after `{`, `(`, `.`, a space, ...

import 'dart:convert';
import 'dart:io';

import 'lsp_server.dart';

/// The completions of the snippets in [snippets], for the documents of [documents].
final class SnippetCompletions extends LspFeature {
  SnippetCompletions(this.snippets, this.documents);

  final List<Snippet> snippets;
  final DocumentStore documents;

  @override
  Map<String, Object?> get capabilities => const {'completionProvider': <String, Object?>{}};

  @override
  Map<String, RequestHandler> get requests => {
    'textDocument/completion': (params) {
      final uri = (params['textDocument'] as Map)['uri'] as String;
      final position = (params['position'] as Map?) ?? const {};
      // Zed asks with TriggerCharacter (2) after a character some server declared (`{`, `.`,
      // `(` of the Dart server) and with Invoked (1, the LSP default) for typed letters and
      // for ctrl-space (or whatever key runs editor::ShowCompletions).
      final context = params['context'] as Map?;
      return {
        'isIncomplete': false,
        'items': completions(
          snippets,
          documents.text(uri),
          (position['line'] as num?)?.toInt() ?? -1,
          (position['character'] as num?)?.toInt() ?? 0,
          invoked: (context?['triggerKind'] as num?)?.toInt() != 2,
        ),
      };
    },
  };
}

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
