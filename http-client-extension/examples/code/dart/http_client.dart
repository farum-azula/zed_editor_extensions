// package:http (Flutter / Dart).
// Click "🌐 Generate .http request" above a call, or press cmd+. on the line.
import 'dart:convert';

import 'package:http/http.dart' as http;

class HttpClientApi {
  HttpClientApi(this.baseUrl);

  final String baseUrl;

  Future<http.Response> item(String id) => http.get(Uri.parse('$baseUrl/items/$id'));

  Future<http.Response> updateItem(Item item) =>
      http.put(Uri.parse('$baseUrl/items/${item.id}'), body: jsonEncode(item.toJson()));

  Future<http.Response> createItem(Item item) => http.post(
        Uri.parse('$baseUrl/items'),
        headers: {'Content-Type': 'application/json', 'X-Trace': 'demo'},
        body: jsonEncode({'title': item.title, 'done': item.done}),
      );

  Future<http.Response> deleteTask(String id) =>
      http.delete(Uri.https('api.example.com', '/v1/tasks/$id'));
}

class Item {
  final int id;
  final String title;
  final bool done;
  final List<String> tags;
}
