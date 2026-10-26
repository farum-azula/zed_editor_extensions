// spring_boot_starter_feign (Feign clients for Dart).
// Click "🌐 Generate .http request" above a mapping, or press cmd+. on the line.
// The request goes where the client does: `url:` with its ${...} placeholders
// resolved from the project's application.yml / application.properties
// (else a {{catalogUrl}}-style variable), plus the client's `path:`.
import 'package:spring_boot_starter_feign/spring_boot_starter_feign.dart';

@FeignClient('catalog', url: r'${catalog.url}', path: '/api')
abstract interface class CatalogClient {
  @GetMapping('/books/{isbn}')
  Future<Book> find(@PathVariable() String isbn);

  @GetMapping('/books')
  Future<List<Book>> search(@RequestParam('q') String query);

  @PostMapping('/books')
  Future<Book> create(@RequestBody() Book book);
}

class Book {
  final String isbn;
  final String title;
  final int stock;
}
