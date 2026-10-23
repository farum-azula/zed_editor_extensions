// Prints the directories flutter_lsp.dart would run build_runner in under a root, one per line,
// for check_lsp.py (the detection in a pure function, without starting a process):
//
//     dart build_runner_projects.dart <root>

import 'dart:io';

import '../tool/flutter_lsp.dart' show buildRunnerProjects;

void main(List<String> args) {
  for (final project in buildRunnerProjects(Directory(args[0]))) {
    print(project.path);
  }
}
