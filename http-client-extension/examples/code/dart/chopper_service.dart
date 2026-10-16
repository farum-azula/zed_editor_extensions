// Chopper.
// Click "🌐 Generate .http request" above a method, or press cmd+. on the line.
import 'package:chopper/chopper.dart';

@ChopperApi(baseUrl: '/todos')
abstract class TodosService extends ChopperService {
  @Get(path: '/{id}')
  Future<Response> getTodo(@Path() String id, @Query('expand') bool expand);

  @Get()
  Future<Response> listTodos(@Query('limit') int limit);

  @Post()
  Future<Response> create(@Header('X-Tenant') String tenant, @Body() Todo todo);
}

class Todo {
  final String title;
  final bool completed;
}
