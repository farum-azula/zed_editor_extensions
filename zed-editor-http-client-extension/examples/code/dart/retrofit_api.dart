// retrofit.dart (code generation on top of Dio).
// Click "🌐 Generate .http request" above a method, or press cmd+. on the line.
import 'package:retrofit/retrofit.dart';

abstract class TaskApi {
  @GET('/tasks/{id}')
  Future<Task> getTask(@Path('id') String id, @Query('expand') bool expand);

  @GET('/tasks')
  Future<List<Task>> listTasks(@Query('status') String status);

  @POST('/tasks')
  Future<Task> createTask(@Header('X-Request-Id') String requestId, @Body() Task task);

  @DELETE('/tasks/{id}')
  Future<void> deleteTask(@Path('id') String id);
}

class Task {
  final String name;
  final int priority;
  final bool done;
  final DateTime dueDate;
}
