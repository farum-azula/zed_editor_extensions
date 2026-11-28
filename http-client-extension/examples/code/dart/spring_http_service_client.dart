// HTTP service clients for Dart (@HttpExchange, spring_boot_starter_restclient).
// Click "🌐 Generate .http request" above an exchange, or press cmd+. on the line.
// The request goes where the client's group does: the group of the
// @ImportHttpServices that lists the interface (in this file or anywhere in the
// project), and its spring.http.serviceclient.<group>.base-url from
// lib/resources/application.yml / application.properties (else a
// {{catalogBaseUrl}}-style variable). The interface's @HttpExchange is the prefix.
import 'package:spring_boot_starter_restclient/spring_boot_starter_restclient.dart';

@HttpExchange('/books')
abstract interface class BookService {
  @GetExchange('/{isbn}')
  Future<Book> find(@PathVariable() String isbn);

  @GetExchange()
  Future<List<Book>> search(
    @RequestParam('q') String query, {
    @RequestParam() int? year,
  });

  @PostExchange()
  Future<ResponseEntity<Book>> create(
    @RequestHeader('X-Request-Id') String requestId,
    @CookieValue('session') String session,
    @RequestBody() Book book,
  );

  @PutExchange.of(url: '/{isbn}/cover', contentType: MediaType.multipartFormData)
  Future<void> uploadCover(
    @PathVariable() String isbn,
    @RequestPart('file') MultipartFile file,
  );

  @DeleteExchange('/{isbn}')
  Future<void> remove(@PathVariable() String isbn);
}

@SpringBootApplication()
@ImportHttpServices(group: 'catalog', types: [BookService])
void main(List<String> args) => runApplication(args);

class Book {
  final String isbn;
  final String title;
  final int stock;
}
