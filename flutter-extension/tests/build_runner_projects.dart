// Prints the build_runner targets under a root, one per line: the directory build_runner runs
// in and its last argument (`--workspace` for a workspace build), for check_lsp.py (the detection
// in a pure function, without starting a process):
//
//     dart build_runner_projects.dart <root>

import 'dart:io';

import '../tool/build_runner.dart';

void main(List<String> args) {
  for (final target in BuildRunnerTarget.under(Directory(args[0]))) {
    print('${target.directory.path} ${target.arguments(BuildRunnerCommand.watch).last}');
  }
}
