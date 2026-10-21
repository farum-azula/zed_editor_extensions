// What the Dartdoc language (languages/dartdoc) makes of `///` comments: a fenced block is
// `dartdoc.code`, and its code is highlighted in the language its fence names (Dart for none, as
// for dartdoc), `keyword.dartdoc` and the like, when it is one of the languages of examples; the
// rest stays a comment. Marks as in highlights.dart, under the `///` line they are about.

/// Aspect-oriented programming: a class with advice.
//                                 ^ comment.documentation
///
/// ```dart
//  ^ dartdoc.code
//     ^ dartdoc.code
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
/// Spring (Java):
/// ```java
/// @Aspect
//   ^ attribute
/// class Auditing {}
//  ^ keyword
//        ^ type
/// ```
///
/// Other languages of examples:
/// ```rust
/// fn audit(calls: u32) -> u32 { calls + 1 }
//  ^ keyword
//     ^ function
//                  ^ type
/// ```
/// ```js
/// const calls = [];
//  ^ keyword
/// ```
/// ```ts
/// interface Clock { now(): Date }
//  ^ keyword
//            ^ type
/// ```
/// ```python
/// def audit(calls):
//  ^ keyword
//      ^ function
///     return calls + 1
//      ^ keyword
/// ```
///
/// A language it has not stays a comment, in a block:
/// ```kotlin
/// fun main() {}
//  ^ dartdoc.code
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
