// The framing the Debug Adapter Protocol and the Language Server Protocol share: a
// `Content-Length` header, a blank line, then that many bytes of JSON.

import 'dart:async';
import 'dart:convert';

List<int> encodeMessage(Map<String, Object?> message) {
  final body = utf8.encode(jsonEncode(message));
  return [...ascii.encode('Content-Length: ${body.length}\r\n\r\n'), ...body];
}

/// Splits a byte stream into its JSON messages.
class DapDecoder extends StreamTransformerBase<List<int>, Map<String, Object?>> {
  @override
  Stream<Map<String, Object?>> bind(Stream<List<int>> stream) async* {
    var buffer = <int>[];
    await for (final chunk in stream) {
      buffer.addAll(chunk);
      while (true) {
        final end = _headerEnd(buffer);
        if (end < 0) {
          break;
        }
        final header = ascii.decode(buffer.sublist(0, end), allowInvalid: true);
        final match = RegExp(r'Content-Length:\s*(\d+)', caseSensitive: false).firstMatch(header);
        if (match == null) {
          throw FormatException('a message without Content-Length: $header');
        }
        final length = int.parse(match.group(1)!);
        final start = end + 4;
        if (buffer.length < start + length) {
          break;
        }
        final body = utf8.decode(buffer.sublist(start, start + length));
        buffer = buffer.sublist(start + length);
        yield (jsonDecode(body) as Map).cast<String, Object?>();
      }
    }
  }

  static int _headerEnd(List<int> buffer) {
    for (var i = 0; i + 3 < buffer.length; i++) {
      if (buffer[i] == 13 && buffer[i + 1] == 10 && buffer[i + 2] == 13 && buffer[i + 3] == 10) {
        return i;
      }
    }
    return -1;
  }
}
