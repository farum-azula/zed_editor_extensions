// Selects a widget of a running Flutter app in its inspector, as a click in DevTools' widget tree
// does: for check_hot_reload.py, which checks that the selection opens the widget's code in Zed.
//
//     dart select_widget.dart <VM service URI> <widget>
//
// Selects the first widget named <widget> that the app's own code creates, and prints its
// description. Only `dart:` libraries, as the tools.

import 'dart:async';
import 'dart:convert';
import 'dart:io';

Future<void> main(List<String> args) async {
  final socket = await WebSocket.connect(args[0]);
  final pending = <int, Completer<Map<String, Object?>>>{};
  socket.listen((data) {
    final message = jsonDecode(data as String) as Map<String, Object?>;
    final completer = pending.remove(message['id']);
    if (message['error'] != null) {
      completer?.completeError(StateError('${message['error']}'));
    } else {
      completer?.complete((message['result'] as Map).cast());
    }
  });
  var id = 0;
  Future<Map<String, Object?>> call(String method, [Map<String, Object?> params = const {}]) {
    final completer = pending[++id] = Completer();
    socket.add(jsonEncode({'jsonrpc': '2.0', 'id': id, 'method': method, 'params': params}));
    return completer.future.timeout(const Duration(seconds: 30));
  }

  const select = 'ext.flutter.inspector.setSelectionById';
  String? isolateId;
  for (final ref in (await call('getVM'))['isolates'] as List) {
    final isolate = await call('getIsolate', {'isolateId': (ref as Map)['id']});
    if ((isolate['extensionRPCs'] as List? ?? const []).contains(select)) {
      isolateId = isolate['id'] as String;
    }
  }
  if (isolateId == null) {
    stderr.writeln('no isolate has the widget inspector');
    exit(1);
  }
  const group = 'zed-flutter-tools-check';
  final tree = await call('ext.flutter.inspector.getRootWidgetTree', {
    'isolateId': isolateId,
    'groupName': group,
    'isSummaryTree': 'true',
    'withPreviews': 'false',
  });
  Map<String, Object?>? find(Map<String, Object?> node) {
    if (node['createdByLocalProject'] == true && node['description'] == args[1]) {
      return node;
    }
    for (final child in node['children'] as List? ?? const []) {
      if (find((child as Map).cast()) case final found?) {
        return found;
      }
    }
    return null;
  }

  final widget = find((tree['result'] as Map).cast());
  if (widget == null) {
    stderr.writeln('no ${args[1]} created by the app');
    exit(1);
  }
  await call(select, {'isolateId': isolateId, 'arg': widget['valueId'], 'objectGroup': group});
  stdout.writeln(widget['description']);
  await socket.close();
}
