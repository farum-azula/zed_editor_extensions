// Opens a Flutter DevTools page for the app running under the debugger, in a window of its own.
//
//     dart flutter_devtools.dart <page> [project directory]
//
// <page> is one of inspector, performance, cpu-profiler, memory, network, logging (see
// devtools.dart). The app is the one the `Flutter` debug adapter runs for the project, or, with
// none for it, the latest one.

import 'dart:io';

import 'devtools.dart';

Future<void> main(List<String> args) async {
  if (args.isEmpty || !pages.containsKey(args.first)) {
    stderr.writeln('usage: dart flutter_devtools.dart <${pages.keys.join('|')}> [project]');
    exit(64);
  }
  final session = sessionFor(await liveSessions(), args.length > 1 ? args[1] : null);
  if (session == null) {
    stderr.writeln(
      'No Flutter app is running under the debugger. Start one with "Flutter: debug on ..." '
      '(debugger: start), then open DevTools again.',
    );
    exit(1);
  }
  final url = session.pageUrl(args.first);
  stdout.writeln('DevTools ${pages[args.first]}: $url');
  await openWindow(url);
}
