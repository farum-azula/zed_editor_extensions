// What highlights.scm makes of Dart: under a line, `^` marks a column of it and names the capture
// expected there (or one it falls back from, `keyword` for `keyword.definition`).

import 'package:flutter/material.dart';
// <- keyword

final class Models({required this.model}) {
//    ^ keyword
//          ^ type
//                  ^ keyword
//                           ^ variable.builtin
  final SceneModel model;
//^ keyword
//      ^ type

  factory neonCat() {
//^ keyword
//        ^ function.method
    final String path = Assets.glb.neonCat;
//        ^ type
//                      ^ type
//                                 ^ property
    return Models(model: SceneModel(path));
//  ^ keyword
//         ^ type
//                ^ variable.parameter
  }
}

class const MyApp({super.key}) extends StatelessWidget {
//    ^ keyword
//          ^ type
//                 ^ keyword
//                              ^ keyword
  @override
//^ attribute
  Widget build(BuildContext context) => const Text('x');
//^ type
//       ^ function.method
//                                      ^ keyword
}

class Old {
  const Old({super.key});
//^ keyword
//           ^ keyword
  Old({required this.x});
//^ type
//     ^ keyword
  factory Old.named() => Old(x: 1);
//        ^ type
//            ^ function.method
  factory Old() => Old.named();
//        ^ type
  static Old make() => Old(x: 2);
//^ keyword
//           ^ function.method
  final int x;
  int get code => super.hashCode;
//                ^ keyword
}

@immutable
// <- attribute
enum Status { active, inactive }
//   ^ type
//            ^ property

void greet(String name) {
//   ^ function.method
//                ^ variable.parameter
  print('Hello, $name');
//^ function
//      ^ string
  final user = Models.neonCat();
//                    ^ function.method
  final QuizModel? quiz = ModalRoute.of<Object?>(ctx)?.settings.arguments as QuizModel?;
//               ^ operator
//                                      ^ type
//                                            ^ operator
//                                                   ^ operator
//                                                                                    ^ operator
  quiz?..reset();
//    ^ operator
  quiz ??= user!;
//     ^ operator
//             ^ operator
}
