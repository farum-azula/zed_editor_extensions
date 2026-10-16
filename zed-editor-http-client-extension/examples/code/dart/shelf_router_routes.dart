// shelf_router (Dart server).
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
import 'package:shelf_router/shelf_router.dart';

Router buildRouter() {
  final app = Router();

  app.get('/hello/<name>', _hello);
  app.get('/users', _users);
  app.post('/users', _createUser);
  app.delete('/users/<id>', _deleteUser);

  return app;
}

Object _hello(request, String name) => 'hi $name';
Object _users(request) => '[]';
Object _createUser(request) => '{}';
Object _deleteUser(request, String id) => '';
