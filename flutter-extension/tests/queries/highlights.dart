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

@meta.immutable
// <- attribute
//^ attribute
//    ^ attribute
class Prefixed {}

void search(
  @RequestParam('q') String query, {
//^ attribute
// ^ attribute
  @RequestParam.of(name: 'page') int page = 0,
//^ attribute
// ^ attribute
//              ^ function.method
  @web.RequestParam.of(name: 'size') int size = 10,
//^ attribute
// ^ attribute
//     ^ attribute
//                  ^ function.method
}) {}

// Built-in words as names (import, get, set) keep the name's colour.
class Importer {
  ImportBatch import(List<String> books) => ImportBatch();
//            ^ function.method
//                                          ^ type
  int get set => 1;
//        ^ property
  void run(Importer other, {bool get = false}) {
//                               ^ variable.parameter
    other.import([]);
//        ^ function.method
    other..import([]);
//         ^ function.method
    final n = other.set;
//                  ^ property
    run(other, get: true);
//             ^ variable.parameter
  }
}

// A variable after `=>` stays a variable.
void listen(List<String> heard, List<String> all) {
  all.forEach((String line) => heard.add(line));
//                             ^ variable
}

// A closure's parameters apart from a function's.
void closures(List<String> all, Map<String, int> counts) {
//                         ^ variable.parameter
  all.forEach((line) => print(line));
//             ^ variable.parameter.closure
  all.forEach((String line) {
//                    ^ variable.parameter.closure
    print(line);
  });
  counts.forEach((key, value) => print(value));
//                ^ variable.parameter.closure
//                     ^ variable.parameter.closure
  final f = ({int size = 1}) => size;
//                ^ variable.parameter.closure
  final g = (int a, [int b = 0]) => a + b;
//               ^ variable.parameter.closure
//                       ^ variable.parameter.closure
}

// Fields and top-level variables where they are declared, as where they are used.
final List<String> events = <String>[];
//                 ^ property
const limit = 10, other = limit;
//    ^ property
//                ^ property
//                        ^ variable
late final Clock clock;
//               ^ property

class Fields {
  static const String name = 'fields';
//                    ^ property
  final SimpleMapScope _scope = SimpleMapScope();
//                     ^ property
  int count = 0, total = count;
//    ^ property
//               ^ property
//                       ^ variable
  late final Clock clock;
//                 ^ property
  void run() {
    final local = 1;
//        ^ variable
    _scope.remove(name);
//  ^ variable
  }
}

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
