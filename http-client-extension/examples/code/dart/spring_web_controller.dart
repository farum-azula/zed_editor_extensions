// spring_boot_starter_web (Spring MVC for Dart).
// Click "🌐 Generate .http request" above a mapping, or press cmd+. on the line.
// The request goes to {{baseUrl}}, which the extension sets from the project's
// lib/resources/application.yml (server.port, server.servlet.context-path).
import 'package:spring_boot_starter_web/spring_boot_starter_web.dart';

@RestController()
@RequestMapping('/api/books')
class BookController {
  BookController(this.books);

  final BookService books;

  @GetMapping('/{isbn}')
  Future<Book> find(@PathVariable() String isbn) => books.find(isbn);

  @GetMapping()
  Future<List<Book>> search(
    @RequestParam('q') String query, {
    @RequestParam.of(name: 'page', defaultValue: '0') int page = 0,
  }) => books.search(query, page: page);

  @PostMapping()
  Future<ResponseEntity<Book>> create(
    @RequestHeader('X-Request-Id') String requestId,
    @RequestBody() @Valid() NewBook book,
  ) async => ResponseEntity.created(Uri.parse('/api/books/${book.isbn}')).body(await books.create(book));

  @DeleteMapping('/{isbn}')
  @ResponseStatus(HttpStatus.noContent)
  void delete(@PathVariable() String isbn) => books.delete(isbn);
}

class NewBook {
  const NewBook({required this.isbn, required this.title, this.stock = 0});

  final String isbn;
  final String title;
  final int stock;
}
