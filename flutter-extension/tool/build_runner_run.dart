// Runs build_runner for the package of a file, as the language server's watches and the
// `dart run` task do (build_runner.dart): with `--workspace` in a pub workspace that builds as one,
// so that the tasks use the build graph of the watch.
//
//     dart build_runner_run.dart <build|watch> <file or directory>
//
// The exit code is build_runner's; 64 for a wrong command line, 66 when nothing builds the package.

import 'dart:io';

import 'build_runner.dart';
import 'pubspec.dart';

Future<void> main(List<String> args) async {
  final command = args.isEmpty ? null : BuildRunnerCommand.values.asNameMap()[args.first];
  if (command == null || args.length != 2) {
    stderr.writeln('usage: dart build_runner_run.dart <build|watch> <file or directory>');
    exit(64);
  }
  final start = FileSystemEntity.isDirectorySync(args[1])
      ? Directory(args[1])
      : File(args[1]).parent;
  final package = packageDirectory(start);
  final target = package == null ? null : BuildRunnerTarget.of(package);
  if (target == null) {
    stderr.writeln(
      'build_runner: nothing builds ${package?.path ?? start.path}: no build_runner in the '
      'dev_dependencies of its package or of its pub workspace',
    );
    exit(66);
  }
  final arguments = target.arguments(command);
  stdout.writeln('dart ${arguments.join(' ')}  (in ${target.directory.path})');
  final process = await Process.start(
    Platform.resolvedExecutable,
    arguments,
    workingDirectory: target.directory.path,
    mode: ProcessStartMode.inheritStdio,
  );
  exit(await process.exitCode);
}
