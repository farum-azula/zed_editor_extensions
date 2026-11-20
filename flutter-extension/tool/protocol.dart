// The framing the Debug Adapter Protocol and the Language Server Protocol share: a
// `Content-Length` header, a blank line, then that many bytes of JSON.

import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

List<int> encodeMessage(Map<String, Object?> message) {
  final body = utf8.encode(jsonEncode(message));
  return [...ascii.encode('Content-Length: ${body.length}\r\n\r\n'), ...body];
}

/// Splits a byte stream into its JSON messages.
class DapDecoder extends StreamTransformerBase<List<int>, Map<String, Object?>> {
  @override
  Stream<Map<String, Object?>> bind(Stream<List<int>> stream) async* {
    final frames = _Frames();
    await for (final chunk in stream) {
      frames.add(chunk);
      for (var body = frames.next(); body != null; body = frames.next()) {
        yield (jsonDecode(utf8.decode(body)) as Map).cast<String, Object?>();
      }
    }
  }
}

/// The bytes received and not yet read as messages. A message is read where it lies, without
/// copying what follows it (a large file the editor sends comes in many chunks).
final class _Frames {
  var _bytes = Uint8List(1 << 16);

  /// Where the unread bytes start and end in [_bytes].
  var _start = 0;
  var _end = 0;

  /// How far the search for the end of the current header got, so that it does not start over
  /// with each chunk.
  var _searched = 0;

  static final _contentLength = RegExp(r'Content-Length:\s*(\d+)', caseSensitive: false);

  void add(List<int> chunk) {
    if (_end + chunk.length > _bytes.length) {
      final unread = _end - _start;
      final grown = unread + chunk.length > _bytes.length ~/ 2
          ? Uint8List((unread + chunk.length) * 2)
          : _bytes;
      grown.setRange(0, unread, _bytes, _start);
      _bytes = grown;
      _searched -= _start;
      _start = 0;
      _end = unread;
    }
    _bytes.setRange(_end, _end + chunk.length, chunk);
    _end += chunk.length;
  }

  /// The body of the next complete message; null until one has arrived whole.
  Uint8List? next() {
    final headerEnd = _headerEnd();
    if (headerEnd < 0) {
      return null;
    }
    final header = ascii.decode(
      Uint8List.sublistView(_bytes, _start, headerEnd),
      allowInvalid: true,
    );
    final match = _contentLength.firstMatch(header);
    if (match == null) {
      throw FormatException('a message without Content-Length: $header');
    }
    final bodyStart = headerEnd + 4;
    final bodyEnd = bodyStart + int.parse(match.group(1)!);
    if (_end < bodyEnd) {
      return null;
    }
    final body = Uint8List.sublistView(_bytes, bodyStart, bodyEnd);
    _start = _searched = bodyEnd;
    return body;
  }

  /// Where the blank line after the current header starts; -1 when it has not arrived.
  int _headerEnd() {
    for (var i = _searched < _start ? _start : _searched; i + 3 < _end; i++) {
      if (_bytes[i] == 13 && _bytes[i + 1] == 10 && _bytes[i + 2] == 13 && _bytes[i + 3] == 10) {
        return i;
      }
    }
    _searched = _end - 3 < _start ? _start : _end - 3;
    return -1;
  }
}
