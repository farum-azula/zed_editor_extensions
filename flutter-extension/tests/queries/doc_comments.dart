// What the Dartdoc language (languages/dartdoc) makes of `///` comments: the code of a fenced
// block that names Dart, or no language, is highlighted as Dart; the rest stays a comment. Marks as
// in highlights.dart, under the `///` line they are about.

/// Aspect-oriented programming: a class with advice.
//                                 ^ comment.documentation
///
/// ```dart
/// @Aspect()
//  ^ attribute
//   ^ attribute
/// class Auditing {
//  ^ keyword
//        ^ type
///   void audit(JoinPoint joinPoint) => print('audit');
//    ^ keyword
//         ^ function.method
//               ^ type
//                                             ^ string
///   // A comment of the example: the next line is code again.
//       ^ comment
///   final int calls = 0;
//    ^ keyword
//          ^ type
/// }
/// ```
///
/// Spring (Java), not Dart:
/// ```java
/// class Auditing {}
//  ^ comment.documentation
/// ```
///
/// A fence without a language is Dart, as for dartdoc:
/// ```
/// final list = <String>[];
//  ^ keyword
//                ^ type
/// ```
/// final after = 'the block';
//  ^ comment.documentation
class Documented {
  /// A member's comment, indented:
  ///
  /// ```dart
  /// const name = 'x';
  //  ^ keyword
  //               ^ string
  /// ```
  String member() => 'x';
}

/** A block comment's code is not highlighted.
 *
 * ```dart
 * final x = 1;
 * ```
 */
void main() {}
