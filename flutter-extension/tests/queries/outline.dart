// What outline.scm lists of Dart: check_queries.py compares it with OUTLINE there.
class Old {
  Old({required this.x});
  Old.named() : x = 1;
  const Old.c() : x = 2;
  factory Old.f() => Old.named();
  factory Old.redirect() = Old.named;
  final int x, y = 1;
  static const a = 1, b = 2;
  void m() { final p = 1, q = 2; }
}
late final int top;
final first = 1, second = 2;
mixin M on Object {}
extension type const Meters(int value) {}
typedef Json = Map<String, Object?>;
enum E { a, b }
