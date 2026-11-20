// A small language server over stdin and stdout: the protocol's lifecycle, and a registry of
// features, each adding its capabilities and the methods it handles. flutter_lsp.dart lists its
// features; a new one is a class of its own and one entry in that list.

import 'dart:async';
import 'dart:io';

import 'protocol.dart';

/// The type of a message to the editor, as the protocol numbers them.
enum MessageType {
  error(1),
  warning(2),
  info(3),
  log(4);

  const MessageType(this.code);

  final int code;
}

/// What a feature can ask of the editor.
abstract interface class LspClient {
  /// Writes [text] into the server's log in the editor.
  void log(MessageType type, String text);

  /// Shows [text] to the user (a notification).
  void show(MessageType type, String text);

  /// Sends the request [method]; its answer is not waited for.
  void request(String method, Map<String, Object?> params);
}

/// The parameters of `initialize` a feature reads.
final class InitializeParams {
  InitializeParams(this._params);

  final Map<String, Object?> _params;

  /// The folders the editor opened: its `workspaceFolders`, else its `rootUri`.
  List<Directory> get workspaceFolders {
    final uris = <String>[
      for (final folder in (_params['workspaceFolders'] as List?) ?? const [])
        if (folder is Map && folder['uri'] is String) folder['uri'] as String,
    ];
    if (uris.isEmpty && _params['rootUri'] is String) {
      uris.add(_params['rootUri'] as String);
    }
    return [
      for (final uri in uris.toSet())
        if (Uri.tryParse(uri) case final parsed? when parsed.scheme == 'file')
          Directory.fromUri(parsed),
    ];
  }

  /// The `initialization_options` of the server's settings in the editor.
  Object? get initializationOptions => _params['initializationOptions'];

  /// Whether the editor lets a server register for changes of files (`**/pubspec.yaml`).
  bool get watchesFiles =>
      _capability(['workspace', 'didChangeWatchedFiles', 'dynamicRegistration']) == true;

  Object? _capability(List<String> path) {
    Object? value = _params['capabilities'];
    for (final key in path) {
      value = value is Map ? value[key] : null;
    }
    return value;
  }
}

/// A request handler: its result, or a [Future] of it.
typedef RequestHandler = FutureOr<Object?> Function(Map<String, Object?> params);

/// A notification handler.
typedef NotificationHandler = void Function(Map<String, Object?> params);

/// One feature of the server. Every member has a default that does nothing.
abstract class LspFeature {
  /// What it adds to the server's capabilities in the answer to `initialize`.
  Map<String, Object?> get capabilities => const {};

  /// The requests it answers, by method.
  Map<String, RequestHandler> get requests => const {};

  /// The notifications it handles, by method.
  Map<String, NotificationHandler> get notifications => const {};

  /// Called with the `initialize` request, before the answer.
  void initialize(InitializeParams params) {}

  /// Called with the `initialized` notification: the editor is ready for requests of the server.
  void initialized(LspClient client) {}

  /// Called once when the server stops (`shutdown`, `exit`, or the editor went away).
  void shutdown() {}
}

/// The server: answers `initialize` with the capabilities of its [features], hands each request
/// and notification to the feature that handles its method, and refuses other requests.
final class LspServer implements LspClient {
  LspServer(this.name, this.features, {Stream<List<int>>? input, IOSink? output})
    : _input = input ?? stdin,
      _output = output ?? stdout {
    for (final feature in features) {
      _requests.addAll(feature.requests);
      for (final MapEntry(key: method, value: handler) in feature.notifications.entries) {
        (_notifications[method] ??= []).add(handler);
      }
    }
  }

  /// The name in `serverInfo`.
  final String name;

  final List<LspFeature> features;
  final Stream<List<int>> _input;
  final IOSink _output;
  final _requests = <String, RequestHandler>{};
  final _notifications = <String, List<NotificationHandler>>{};
  var _nextRequest = 0;
  var _stopped = false;

  /// Handles messages until `exit` or the end of the input; the exit code.
  Future<int> serve() async {
    await for (final message in _input.transform(DapDecoder())) {
      if (await _handle(message) case final code?) {
        return code;
      }
    }
    _stop();
    return 0;
  }

  /// Handles [message]; the exit code once the editor asked to exit.
  Future<int?> _handle(Map<String, Object?> message) async {
    final id = message['id'];
    final method = message['method'];
    final params = (message['params'] as Map?)?.cast<String, Object?>() ?? const {};
    switch (method) {
      case null:
        return null; // the answer to a request of this server
      case 'initialize':
        final initialize = InitializeParams(params);
        for (final feature in features) {
          feature.initialize(initialize);
        }
        _send({
          'id': id,
          'result': {
            'capabilities': {for (final feature in features) ...feature.capabilities},
            'serverInfo': {'name': name},
          },
        });
      case 'initialized':
        for (final feature in features) {
          feature.initialized(this);
        }
      case 'shutdown':
        _stop();
        _send({'id': id, 'result': null});
      case 'exit':
        _stop();
        return 0;
      case final String method when id != null:
        await _answer(id, method, params);
      case final String method:
        for (final handler in _notifications[method] ?? const <NotificationHandler>[]) {
          handler(params);
        }
    }
    return null;
  }

  Future<void> _answer(Object id, String method, Map<String, Object?> params) async {
    final handler = _requests[method];
    if (handler == null) {
      _send({
        'id': id,
        'error': {'code': -32601, 'message': 'not supported'},
      });
      return;
    }
    try {
      _send({'id': id, 'result': await handler(params)});
    } on Object catch (error) {
      _send({
        'id': id,
        'error': {'code': -32603, 'message': '$error'},
      });
    }
  }

  void _stop() {
    if (_stopped) {
      return;
    }
    _stopped = true;
    for (final feature in features) {
      feature.shutdown();
    }
  }

  @override
  void log(MessageType type, String text) => _send({
    'method': 'window/logMessage',
    'params': {'type': type.code, 'message': text},
  });

  @override
  void show(MessageType type, String text) => _send({
    'method': 'window/showMessage',
    'params': {'type': type.code, 'message': text},
  });

  @override
  void request(String method, Map<String, Object?> params) =>
      _send({'id': 'flutter-tools-${_nextRequest++}', 'method': method, 'params': params});

  void _send(Map<String, Object?> message) =>
      _output.add(encodeMessage({'jsonrpc': '2.0', ...message}));
}

/// Something that wants to know which documents are open.
/// Told about the documents the editor opens, saves and closes; each does nothing unless
/// overridden.
abstract class DocumentObserver {
  void opened(Uri uri) {}

  void saved(Uri uri) {}

  void closed(Uri uri) {}
}

/// The text of the documents open in the editor, kept up to date with its changes (incremental:
/// the editor sends only what changed).
final class DocumentStore extends LspFeature {
  final _texts = <String, String>{};
  final _observers = <DocumentObserver>[];

  /// Tells [observer] about documents opened, saved and closed from now on.
  void observe(DocumentObserver observer) => _observers.add(observer);

  /// The text of the document [uri]; empty when it is not open.
  String text(String uri) => _texts[uri] ?? '';

  @override
  Map<String, Object?> get capabilities => const {
    'textDocumentSync': {
      'openClose': true,
      'change': 2, // incremental
      'save': {'includeText': false},
    },
  };

  @override
  Map<String, NotificationHandler> get notifications => {
    'textDocument/didOpen': (params) {
      final document = params['textDocument'] as Map;
      final uri = document['uri'] as String;
      _texts[uri] = document['text'] as String;
      for (final observer in _observers) {
        observer.opened(Uri.parse(uri));
      }
    },
    'textDocument/didChange': (params) {
      final uri = (params['textDocument'] as Map)['uri'] as String;
      for (final change in (params['contentChanges'] as List).cast<Map>()) {
        _texts[uri] = applyChange(_texts[uri] ?? '', change);
      }
    },
    'textDocument/didSave': (params) {
      final uri = Uri.parse((params['textDocument'] as Map)['uri'] as String);
      for (final observer in _observers) {
        observer.saved(uri);
      }
    },
    'textDocument/didClose': (params) {
      final uri = (params['textDocument'] as Map)['uri'] as String;
      _texts.remove(uri);
      for (final observer in _observers) {
        observer.closed(Uri.parse(uri));
      }
    },
  };
}

/// [text] with [change] (a `TextDocumentContentChangeEvent`) applied: its `range` replaced by its
/// `text`, or the whole text when it has no range.
String applyChange(String text, Map<Object?, Object?> change) {
  final replacement = change['text'] as String;
  final range = change['range'] as Map?;
  if (range == null) {
    return replacement;
  }
  final start = _offset(text, range['start'] as Map);
  final end = _offset(text, range['end'] as Map);
  return text.replaceRange(start, end < start ? start : end, replacement);
}

/// The offset in [text] of a protocol position: a line from 0 and a character in UTF-16 units,
/// clamped to the text.
int _offset(String text, Map<Object?, Object?> position) {
  final line = (position['line'] as num).toInt();
  final character = (position['character'] as num).toInt();
  var offset = 0;
  for (var current = 0; current < line; current++) {
    final next = text.indexOf('\n', offset);
    if (next < 0) {
      return text.length;
    }
    offset = next + 1;
  }
  final lineEnd = switch (text.indexOf('\n', offset)) {
    -1 => text.length,
    final end => end,
  };
  return offset + character > lineEnd ? lineEnd : offset + character;
}
