// Runs the Flutter app of the current directory on a run target, in the terminal: `flutter run`
// with its keys (r hot reload, R hot restart, q quit), on a device it picks or starts first.
//
//     dart flutter_run.dart <target> [flutter run arguments...]
//
// <target> is one of ios-simulator, android, web, macos, windows, linux (see devices.dart).

import 'dart:io';

import 'devices.dart';

Future<void> main(List<String> args) async {
  if (args.isEmpty) {
    stderr.writeln(
      'usage: dart flutter_run.dart <${targets.keys.join('|')}> [flutter run args...]',
    );
    exit(64);
  }
  final flutter = flutterCommand(Directory.current);
  final String device;
  try {
    device = await resolveDevice(args.first, flutter, stdout.writeln);
  } on DeviceException catch (error) {
    stderr.writeln(error);
    exit(1);
  }
  final run = await Process.start(
    flutter.first,
    [...flutter.skip(1), 'run', '-d', device, ...args.skip(1)],
    mode: ProcessStartMode.inheritStdio,
    runInShell: Platform.isWindows,
  );
  exit(await run.exitCode);
}
