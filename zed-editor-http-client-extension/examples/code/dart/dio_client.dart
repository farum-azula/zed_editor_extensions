// Dio (Flutter).
// Click "🌐 Generate .http request" above a call, or press cmd+. on the line.
import 'package:dio/dio.dart';

class DioClient {
  DioClient(this._dio, this.token);

  final Dio _dio;
  final String token;

  Future<Response> login(String email, String password) =>
      _dio.post<Map<String, dynamic>>('/auth/login', data: {'email': email, 'password': password});

  Future<Response> users(int page) => _dio.get('/users',
      queryParameters: {'page': page, 'size': 20},
      options: Options(headers: {'Authorization': 'Bearer $token', 'X-App-Version': '1.0'}));

  Future<Response> updateUser(String id, User user) => _dio.put('/users/$id', data: user.toJson());

  Future<Response> deleteUser(String id) => _dio.delete('/users/$id');
}

class User {
  final String name;
  final String email;
  final int age;
  final bool active;
}
